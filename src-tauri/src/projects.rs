//! Catálogo de projetos da GUI (apenas referências locais) e inspeção de pastas/marker.

use crate::paths::strip_verbatim;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub display_name: String,
    pub absolute_path: String,
    pub added_at: String,
    #[serde(default)]
    pub last_doctor_at: Option<String>,
    #[serde(default)]
    pub last_workspace: Option<String>,
    #[serde(default)]
    pub last_project: Option<String>,
}

pub struct Catalog {
    file: PathBuf,
}

impl Catalog {
    pub fn new(app_dir: &Path) -> Self {
        Self { file: app_dir.join("projects.json") }
    }
    pub fn load(&self) -> Vec<Project> {
        std::fs::read(&self.file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, list: &[Project]) -> std::io::Result<()> {
        crate::backup::write_atomic(&self.file, &serde_json::to_vec_pretty(list).unwrap_or_default())
    }
    pub fn add(&self, path: &Path, display_name: Option<String>) -> Result<Project, String> {
        let mut list = self.load();
        let norm = path.to_string_lossy().to_lowercase();
        if let Some(p) = list.iter().find(|p| p.absolute_path.to_lowercase() == norm) {
            return Err(format!("Já está no catálogo como \"{}\".", p.display_name));
        }
        let name = display_name.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
            path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string_lossy().to_string())
        });
        let p = Project {
            id: uuid::Uuid::new_v4().to_string(),
            display_name: name,
            absolute_path: path.to_string_lossy().to_string(),
            added_at: chrono::Local::now().to_rfc3339(),
            last_doctor_at: None,
            last_workspace: None,
            last_project: None,
        };
        list.push(p.clone());
        self.save(&list).map_err(|e| e.to_string())?;
        Ok(p)
    }
    /// Remove apenas do catálogo da GUI. Nunca toca memórias, marker ou configs.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut list = self.load();
        let before = list.len();
        list.retain(|p| p.id != id);
        if list.len() == before {
            return Err("Projeto não encontrado no catálogo.".into());
        }
        self.save(&list).map_err(|e| e.to_string())
    }
    pub fn update<F: FnOnce(&mut Project)>(&self, id: &str, f: F) -> Result<Project, String> {
        let mut list = self.load();
        let p = list.iter_mut().find(|p| p.id == id).ok_or("Projeto não encontrado.")?;
        f(p);
        let out = p.clone();
        self.save(&list).map_err(|e| e.to_string())?;
        Ok(out)
    }
    pub fn get(&self, id: &str) -> Option<Project> {
        self.load().into_iter().find(|p| p.id == id)
    }
}

// ---------------------------------------------------------------- inspeção de pasta

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubRepo {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerInfo {
    pub path: String,
    pub in_selected_folder: bool,
    pub workspace: Option<String>,
    pub project: Option<String>,
    pub identity: Option<String>,
    pub capture_only: bool,
    pub has_ignore_paths: bool,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInspection {
    pub input: String,
    pub resolved_path: String,
    pub is_dir: bool,
    pub is_git: bool,
    pub git_root: Option<String>,
    pub git_root_differs: bool,
    pub sub_repos: Vec<SubRepo>,
    pub marker: Option<MarkerInfo>,
    pub suggested_workspace: String,
    pub suggested_project: String,
    pub warnings: Vec<String>,
}

/// Valida e normaliza um caminho absoluto escolhido pelo usuário.
pub fn normalize_folder(input: &str) -> Result<PathBuf, String> {
    let trimmed = input.trim().trim_matches('"');
    if trimmed.is_empty() {
        return Err("Informe uma pasta.".into());
    }
    let p = PathBuf::from(trimmed);
    if !p.is_absolute() {
        return Err("O caminho precisa ser absoluto (ex.: F:\\repos\\projeto).".into());
    }
    let canon = std::fs::canonicalize(&p).map_err(|e| format!("Pasta inacessível: {e}"))?;
    let canon = strip_verbatim(&canon);
    if !canon.is_dir() {
        return Err("O caminho não é uma pasta.".into());
    }
    Ok(canon)
}

fn has_git(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(d) = cur {
        if has_git(d) {
            return Some(d.to_path_buf());
        }
        cur = d.parent();
    }
    None
}

static NAME_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._\-]{0,127}$").unwrap());

pub fn valid_name(s: &str) -> bool {
    NAME_RE.is_match(s)
}

/// Sugere nome de projeto a partir do basename (o ai-memory usa remote git quando existe;
/// o valor definitivo é sempre o que o `doctor` informa).
pub fn suggest_project_name(dir: &Path) -> String {
    let base = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "projeto".into());
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '-' })
        .collect();
    let cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() { "projeto".into() } else { cleaned }
}

pub fn read_marker(path: &Path, selected: &Path) -> MarkerInfo {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut m = MarkerInfo {
        path: path.to_string_lossy().to_string(),
        in_selected_folder: path.parent() == Some(selected),
        workspace: None,
        project: None,
        identity: None,
        capture_only: false,
        has_ignore_paths: false,
        parse_error: None,
    };
    match text.parse::<toml::Table>() {
        Ok(t) => {
            m.workspace = t.get("workspace").and_then(|v| v.as_str()).map(str::to_string);
            m.project = t.get("project").and_then(|v| v.as_str()).map(str::to_string);
            m.identity = t.get("identity").and_then(|v| v.as_str()).map(str::to_string);
            m.has_ignore_paths = t
                .get("capture")
                .and_then(|c| c.get("ignore_paths"))
                .is_some();
            m.capture_only = m.workspace.is_none() && m.project.is_none() && m.identity.is_none();
        }
        Err(e) => m.parse_error = Some(e.to_string()),
    }
    m
}

/// Procura o marker mais próximo subindo até o HOME (ou raiz da unidade/checkout).
pub fn find_marker(start: &Path, home: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(d) = cur {
        let m = d.join(".ai-memory.toml");
        if m.is_file() {
            return Some(m);
        }
        if d == home {
            break;
        }
        cur = d.parent();
    }
    None
}

pub fn inspect_folder(input: &str, home: &Path) -> Result<FolderInspection, String> {
    let dir = normalize_folder(input)?;
    let git_root = find_git_root(&dir);
    let mut sub_repos = vec![];
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && has_git(&p) {
                sub_repos.push(SubRepo {
                    name: e.file_name().to_string_lossy().to_string(),
                    path: p.to_string_lossy().to_string(),
                });
            }
        }
    }
    sub_repos.sort_by(|a, b| a.name.cmp(&b.name));
    let marker = find_marker(&dir, home).map(|m| read_marker(&m, &dir));
    let mut warnings = vec![];
    let is_git = git_root.is_some();
    let git_root_differs = git_root.as_ref().map(|g| g != &dir).unwrap_or(false);
    if !is_git && !sub_repos.is_empty() {
        warnings.push(format!(
            "A pasta não é um repositório git, mas contém {} sub-repositório(s). O projeto será esta pasta; sessões abertas dentro de um sub-repo podem ser registradas com outro nome (veja o doctor de cada um).",
            sub_repos.len()
        ));
    }
    if git_root_differs {
        warnings.push("A pasta fica dentro de um repositório git maior; o ai-memory pode resolver o projeto pelo remote desse repositório.".into());
    }
    if let Some(m) = &marker {
        if !m.in_selected_folder {
            warnings.push(format!("Há um marker em pasta ancestral ({}) que vale para esta pasta.", m.path));
        }
        if let Some(e) = &m.parse_error {
            warnings.push(format!("Marker inválido: {e}"));
        }
    }
    Ok(FolderInspection {
        input: input.to_string(),
        resolved_path: dir.to_string_lossy().to_string(),
        is_dir: true,
        is_git,
        git_root: git_root.map(|g| g.to_string_lossy().to_string()),
        git_root_differs,
        sub_repos,
        suggested_workspace: marker.as_ref().and_then(|m| m.workspace.clone()).unwrap_or_else(|| "default".into()),
        suggested_project: marker
            .as_ref()
            .and_then(|m| m.project.clone())
            .unwrap_or_else(|| suggest_project_name(&dir)),
        marker,
        warnings,
    })
}

// ---------------------------------------------------------------- marker

/// Conteúdo do marker gerado pela GUI: apenas chaves documentadas (`workspace`, `project`).
pub fn marker_content(workspace: &str, project: &str) -> Result<String, String> {
    if !valid_name(workspace) {
        return Err("Workspace inválido: use letras, números, '.', '_' ou '-' (até 128).".into());
    }
    if !valid_name(project) {
        return Err("Projeto inválido: use letras, números, '.', '_' ou '-' (até 128).".into());
    }
    Ok(format!(
        "# Gerado pelo AI Memory Manager. Formato: docs/marker-file.md do ai-memory.\nworkspace = \"{workspace}\"\nproject = \"{project}\"\n"
    ))
}

/// Cria o marker sem nunca sobrescrever (create_new).
pub fn write_marker(dir: &Path, content: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    let p = dir.join(".ai-memory.toml");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&p)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                "Já existe um .ai-memory.toml nesta pasta; ele não será sobrescrito.".to_string()
            } else {
                format!("Não foi possível criar o marker: {e}")
            }
        })?;
    f.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_and_missing() {
        assert!(normalize_folder("relativo\\x").is_err());
        assert!(normalize_folder("C:\\nao\\existe\\mesmo").is_err());
        assert!(normalize_folder("").is_err());
    }

    #[test]
    fn inspect_non_git_with_subrepos_and_accents() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("Projeto Açaí");
        std::fs::create_dir_all(dir.join("api").join(".git")).unwrap();
        std::fs::create_dir_all(dir.join("web").join(".git")).unwrap();
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        let i = inspect_folder(&format!("\"{}\"", dir.display()), root.path()).unwrap();
        assert!(!i.is_git);
        assert_eq!(i.sub_repos.len(), 2);
        assert!(i.warnings.iter().any(|w| w.contains("sub-repositório")));
        assert_eq!(i.suggested_workspace, "default");
        assert_eq!(i.suggested_project, "Projeto-A-a"); // não-ASCII vira '-'
        assert!(i.marker.is_none());
    }

    #[test]
    fn inspect_git_subfolder_detects_root() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("pkg")).unwrap();
        let i = inspect_folder(&repo.join("pkg").to_string_lossy(), root.path()).unwrap();
        assert!(i.is_git && i.git_root_differs);
    }

    #[test]
    fn marker_never_overwritten_and_validated() {
        let d = tempfile::tempdir().unwrap();
        let c = marker_content("default", "meu-proj").unwrap();
        assert!(c.contains("workspace = \"default\"") && c.contains("project = \"meu-proj\""));
        write_marker(d.path(), &c).unwrap();
        std::fs::write(d.path().join(".ai-memory.toml"), "workspace = \"orig\"\n").unwrap();
        let err = write_marker(d.path(), &c).unwrap_err();
        assert!(err.contains("não será sobrescrito"));
        assert_eq!(std::fs::read_to_string(d.path().join(".ai-memory.toml")).unwrap(), "workspace = \"orig\"\n");
        assert!(marker_content("default", "nome com espaço").is_err());
        assert!(marker_content("default", "x\"; rm").is_err());
    }

    #[test]
    fn marker_in_ancestor_found_and_capture_only_flag() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        std::fs::create_dir_all(a.join("b")).unwrap();
        std::fs::write(a.join(".ai-memory.toml"), "[capture]\nignore_paths = [\"**/.env\"]\n").unwrap();
        let i = inspect_folder(&a.join("b").to_string_lossy(), root.path()).unwrap();
        let m = i.marker.unwrap();
        assert!(!m.in_selected_folder && m.capture_only && m.has_ignore_paths);
    }

    #[test]
    fn catalog_add_remove_does_not_touch_folder() {
        let d = tempfile::tempdir().unwrap();
        let proj = d.path().join("p");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join(".ai-memory.toml"), "workspace = \"w\"\n").unwrap();
        let cat = Catalog::new(d.path());
        let p = cat.add(&proj, None).unwrap();
        assert!(cat.add(&proj, None).is_err());
        cat.remove(&p.id).unwrap();
        assert!(cat.load().is_empty());
        assert!(proj.join(".ai-memory.toml").exists());
    }
}
