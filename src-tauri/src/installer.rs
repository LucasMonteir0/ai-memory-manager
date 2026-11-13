//! Instalação do ai-memory a partir da release oficial, com verificação SHA-256
//! e extração transacional (staging → rename). Nunca executa install.ps1 remoto.

use crate::aimemory::{self, Semver};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const OFFICIAL_REPO: &str = "akitaonrails/ai-memory";
pub const WINDOWS_ASSET: &str = "ai-memory-windows-x86_64.zip";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub repo: String,
    pub tag: String,
    pub version: Option<String>,
    pub published_at: Option<String>,
    pub html_url: String,
    pub changelog: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_size: Option<u64>,
    pub sha256_url: Option<String>,
    pub publisher: String,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("ai-memory-manager/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("client")
}

pub async fn latest_release() -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{OFFICIAL_REPO}/releases/latest");
    let v: Value = client()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Sem acesso ao GitHub: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub respondeu erro: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Resposta inesperada do GitHub: {e}"))?;
    parse_release(&v)
}

pub fn parse_release(v: &Value) -> Result<ReleaseInfo, String> {
    let tag = v.get("tag_name").and_then(Value::as_str).ok_or("release sem tag_name")?.to_string();
    let assets = v.get("assets").and_then(Value::as_array).cloned().unwrap_or_default();
    let find = |name: &str| assets.iter().find(|a| a.get("name").and_then(Value::as_str) == Some(name)).cloned();
    let zip = find(WINDOWS_ASSET);
    let sha = find(&format!("{WINDOWS_ASSET}.sha256"));
    let url_of = |a: &Option<Value>| a.as_ref().and_then(|a| a.get("browser_download_url")).and_then(Value::as_str).map(str::to_string);
    let asset_url = url_of(&zip);
    let sha256_url = url_of(&sha);
    for u in [&asset_url, &sha256_url].into_iter().flatten() {
        if !u.starts_with(&format!("https://github.com/{OFFICIAL_REPO}/releases/download/")) {
            return Err(format!("URL de asset fora do repositório oficial: {u}"));
        }
    }
    Ok(ReleaseInfo {
        repo: OFFICIAL_REPO.into(),
        version: Semver::parse(&tag).map(|s| s.to_string()),
        tag,
        published_at: v.get("published_at").and_then(Value::as_str).map(str::to_string),
        html_url: v.get("html_url").and_then(Value::as_str).unwrap_or("").to_string(),
        changelog: v.get("body").and_then(Value::as_str).unwrap_or("").chars().take(8000).collect(),
        asset_name: zip.as_ref().map(|_| WINDOWS_ASSET.to_string()),
        asset_size: zip.as_ref().and_then(|a| a.get("size")).and_then(Value::as_u64),
        asset_url,
        sha256_url,
        publisher: OFFICIAL_REPO.split('/').next().unwrap_or("").into(),
    })
}

/// Extrai o hash do arquivo `.sha256` ("<hex>  nome" ou só "<hex>").
pub fn parse_sha256_file(text: &str) -> Option<String> {
    let tok = text.split_whitespace().next()?.to_lowercase();
    (tok.len() == 64 && tok.chars().all(|c| c.is_ascii_hexdigit())).then_some(tok)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub installed_to: String,
    pub version: String,
    pub sha256: String,
    pub added_to_path: bool,
    pub steps: Vec<String>,
}

/// Valida o zip (sem path traversal), localiza ai-memory.exe e extrai em `staging`.
pub fn extract_zip_safely(zip_bytes: &[u8], staging: &Path) -> Result<PathBuf, String> {
    let mut za = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| format!("zip inválido: {e}"))?;
    let mut exe_rel: Option<PathBuf> = None;
    for i in 0..za.len() {
        let mut f = za.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else {
            return Err(format!("Entrada insegura no zip: {}", f.name()));
        };
        let out = staging.join(&rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&out, &buf).map_err(|e| e.to_string())?;
        if rel.file_name().map(|n| n.eq_ignore_ascii_case("ai-memory.exe")).unwrap_or(false) {
            exe_rel = Some(rel);
        }
    }
    let rel = exe_rel.ok_or("ai-memory.exe não encontrado no pacote.")?;
    // raiz = pasta que contém o exe
    Ok(staging.join(rel.parent().unwrap_or_else(|| Path::new(""))))
}

pub async fn install(release: &ReleaseInfo, target_dir: &Path, add_to_path: bool) -> Result<InstallReport, String> {
    if target_dir.join("ai-memory.exe").exists() {
        return Err("Já existe ai-memory.exe no destino. Use 'Atualizar' em vez de instalar.".into());
    }
    let asset = release.asset_url.as_ref().ok_or("Release sem pacote Windows.")?;
    let sha_url = release.sha256_url.as_ref().ok_or("Release sem arquivo .sha256 publicado; instalação recusada.")?;
    let mut steps = vec![];
    let c = client();
    let sha_text = c.get(sha_url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.text().await.map_err(|e| e.to_string())?;
    let expected = parse_sha256_file(&sha_text).ok_or("Arquivo .sha256 em formato inesperado.")?;
    steps.push(format!("SHA-256 publicado: {expected}"));
    let bytes = c.get(asset).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != expected {
        return Err(format!("SHA-256 não confere (esperado {expected}, obtido {actual}). Nada foi instalado."));
    }
    steps.push(format!("Download verificado ({} bytes).", bytes.len()));
    let parent = target_dir.parent().ok_or("destino inválido")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let staging = parent.join(format!(".ai-memory-staging-{}", &uuid::Uuid::new_v4().to_string()[..8]));
    let root = match extract_zip_safely(&bytes, &staging) {
        Ok(r) => r,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };
    let ver = match aimemory::validate_binary(&root.join("ai-memory.exe")).await {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("Binário extraído não validou: {e}"));
        }
    };
    steps.push(format!("Binário validado: ai-memory {}", ver.to_string()));
    if target_dir.exists() {
        // diretório vazio/sem exe: remove só se vazio
        if std::fs::read_dir(target_dir).map(|mut d| d.next().is_some()).unwrap_or(true) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("{} já existe e não está vazio.", target_dir.display()));
        }
        let _ = std::fs::remove_dir(target_dir);
    }
    std::fs::rename(&root, target_dir).map_err(|e| format!("Falha ao mover para o destino: {e}"))?;
    let _ = std::fs::remove_dir_all(&staging);
    steps.push(format!("Instalado em {}", target_dir.display()));
    let mut added = false;
    if add_to_path {
        match add_to_user_path(target_dir) {
            Ok(true) => {
                added = true;
                steps.push("Adicionado ao PATH do usuário (novos terminais/apps).".into());
            }
            Ok(false) => steps.push("Já estava no PATH do usuário.".into()),
            Err(e) => steps.push(format!("Não foi possível alterar o PATH: {e}")),
        }
    }
    Ok(InstallReport {
        installed_to: target_dir.to_string_lossy().to_string(),
        version: ver.to_string(),
        sha256: actual,
        added_to_path: added,
        steps,
    })
}

#[cfg(windows)]
fn add_to_user_path(dir: &Path) -> Result<bool, String> {
    let cur = crate::service::reg_get("Environment", "Path").unwrap_or_default();
    let d = dir.to_string_lossy().to_string();
    if cur.split(';').any(|p| p.trim_end_matches('\\').eq_ignore_ascii_case(d.trim_end_matches('\\'))) {
        return Ok(false);
    }
    let new = if cur.is_empty() { d } else { format!("{};{}", cur.trim_end_matches(';'), d) };
    // REG_EXPAND_SZ preservado
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    let k = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Environment", KEY_SET_VALUE).map_err(|e| e.to_string())?;
    let val = winreg::RegValue {
        vtype: winreg::enums::RegType::REG_EXPAND_SZ,
        bytes: new.encode_utf16().chain(std::iter::once(0)).flat_map(|u| u.to_le_bytes()).collect(),
    };
    k.set_raw_value("Path", &val).map_err(|e| e.to_string())?;
    Ok(true)
}
#[cfg(not(windows))]
fn add_to_user_path(_dir: &Path) -> Result<bool, String> {
    Err("somente Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn release_parsing_requires_official_urls() {
        let ok = json!({"tag_name": "v3.4.6", "html_url": "https://github.com/akitaonrails/ai-memory/releases/tag/v3.4.6",
            "assets": [
              {"name": "ai-memory-windows-x86_64.zip", "size": 10, "browser_download_url": "https://github.com/akitaonrails/ai-memory/releases/download/v3.4.6/ai-memory-windows-x86_64.zip"},
              {"name": "ai-memory-windows-x86_64.zip.sha256", "browser_download_url": "https://github.com/akitaonrails/ai-memory/releases/download/v3.4.6/ai-memory-windows-x86_64.zip.sha256"}
            ]});
        let r = parse_release(&ok).unwrap();
        assert_eq!(r.version.as_deref(), Some("3.4.6"));
        assert!(r.sha256_url.is_some());
        let bad = json!({"tag_name": "v1.0.0", "assets": [
            {"name": "ai-memory-windows-x86_64.zip", "browser_download_url": "https://evil.example/ai-memory.zip"}]});
        assert!(parse_release(&bad).is_err());
    }

    #[test]
    fn sha256_file_formats() {
        let h = "a".repeat(64);
        assert_eq!(parse_sha256_file(&format!("{h}  ai-memory.zip\n")), Some(h.clone()));
        assert_eq!(parse_sha256_file(&h.to_uppercase()), Some(h));
        assert!(parse_sha256_file("nope").is_none());
    }

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            for (n, data) in entries {
                w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn zip_extraction_finds_exe_in_subfolder() {
        let d = tempfile::tempdir().unwrap();
        let z = make_zip(&[("pkg/ai-memory.exe", b"MZ"), ("pkg/hooks/x.ps1", b"x")]);
        let root = extract_zip_safely(&z, d.path()).unwrap();
        assert!(root.join("ai-memory.exe").is_file() && root.join("hooks").join("x.ps1").is_file());
    }

    #[test]
    fn zip_without_exe_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let z = make_zip(&[("readme.txt", b"x")]);
        assert!(extract_zip_safely(&z, d.path()).is_err());
    }

    #[test]
    fn zip_with_traversal_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let z = make_zip(&[("../evil.exe", b"MZ")]);
        assert!(extract_zip_safely(&z, d.path()).is_err());
    }
}
