//! Backups/snapshots dos arquivos que a GUI altera, com diff e rollback.

use crate::sanitize::sanitize;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub original_path: String,
    pub backup_name: Option<String>,
    pub existed: bool,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSet {
    pub id: String,
    pub created_at: String,
    pub reason: String,
    pub agent_id: Option<String>,
    pub files: Vec<BackupFile>,
    /// valores de registro (ex.: Run\ai-memory)
    #[serde(default)]
    pub registry: Vec<RegistryBackup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryBackup {
    pub key: String,
    pub name: String,
    pub value: Option<String>,
    /// valor secreto: nunca gravado; restauração apenas remove o que a GUI criou
    #[serde(default)]
    pub secret: bool,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub struct BackupStore {
    pub root: PathBuf,
}

impl BackupStore {
    pub fn new(app_dir: &Path) -> Self {
        Self { root: app_dir.join("backups") }
    }

    pub fn create(
        &self,
        reason: &str,
        agent_id: Option<&str>,
        files: &[PathBuf],
        registry: Vec<RegistryBackup>,
    ) -> std::io::Result<BackupSet> {
        let id = format!(
            "{}-{}",
            chrono::Local::now().format("%Y%m%d-%H%M%S"),
            &uuid::Uuid::new_v4().to_string()[..8]
        );
        let dir = self.root.join(&id);
        std::fs::create_dir_all(&dir)?;
        let mut out = Vec::new();
        for (i, f) in files.iter().enumerate() {
            if f.is_file() {
                let bytes = std::fs::read(f)?;
                let name = format!(
                    "{i:02}-{}",
                    f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                );
                std::fs::write(dir.join(&name), &bytes)?;
                out.push(BackupFile {
                    original_path: f.to_string_lossy().to_string(),
                    backup_name: Some(name),
                    existed: true,
                    sha256: Some(sha256_hex(&bytes)),
                });
            } else {
                out.push(BackupFile {
                    original_path: f.to_string_lossy().to_string(),
                    backup_name: None,
                    existed: false,
                    sha256: None,
                });
            }
        }
        let set = BackupSet {
            id,
            created_at: chrono::Local::now().to_rfc3339(),
            reason: reason.to_string(),
            agent_id: agent_id.map(str::to_string),
            files: out,
            registry,
        };
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec_pretty(&set).unwrap_or_default(),
        )?;
        Ok(set)
    }

    pub fn list(&self) -> Vec<BackupSet> {
        let mut v: Vec<BackupSet> = std::fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| std::fs::read(e.path().join("manifest.json")).ok())
            .filter_map(|b| serde_json::from_slice(&b).ok())
            .collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    pub fn get(&self, id: &str) -> Option<BackupSet> {
        if id.contains(['/', '\\', '.']) {
            return None;
        }
        std::fs::read(self.root.join(id).join("manifest.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    pub fn backup_content(&self, set: &BackupSet, f: &BackupFile) -> Option<String> {
        let name = f.backup_name.as_ref()?;
        std::fs::read_to_string(self.root.join(&set.id).join(name)).ok()
    }

    /// Restaura arquivos: os que existiam voltam ao conteúdo salvo; os que não existiam são removidos.
    pub fn restore_files(&self, set: &BackupSet) -> std::io::Result<Vec<String>> {
        let mut done = vec![];
        for f in &set.files {
            let target = PathBuf::from(&f.original_path);
            if f.existed {
                let name = f.backup_name.as_ref().expect("backup_name");
                let bytes = std::fs::read(self.root.join(&set.id).join(name))?;
                if let Some(p) = target.parent() {
                    std::fs::create_dir_all(p)?;
                }
                write_atomic(&target, &bytes)?;
                done.push(format!("restaurado: {}", f.original_path));
            } else if target.exists() {
                std::fs::remove_file(&target)?;
                done.push(format!("removido (não existia antes): {}", f.original_path));
            }
        }
        Ok(done)
    }

    /// Diff entre o backup e o estado atual.
    pub fn diff_against_current(&self, set: &BackupSet) -> Vec<FileDiff> {
        set.files
            .iter()
            .map(|f| {
                let before = self.backup_content(set, f).unwrap_or_default();
                let after = std::fs::read_to_string(&f.original_path).unwrap_or_default();
                FileDiff::new(&f.original_path, &before, &after, f.existed)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub existed_before: bool,
    pub changed: bool,
    pub unified: String,
    pub added: usize,
    pub removed: usize,
}

impl FileDiff {
    pub fn new(path: &str, before: &str, after: &str, existed_before: bool) -> Self {
        let diff = similar::TextDiff::from_lines(before, after);
        let mut added = 0;
        let mut removed = 0;
        for c in diff.iter_all_changes() {
            match c.tag() {
                similar::ChangeTag::Insert => added += 1,
                similar::ChangeTag::Delete => removed += 1,
                _ => {}
            }
        }
        let unified = diff
            .unified_diff()
            .context_radius(3)
            .header("antes", "depois")
            .to_string();
        Self {
            path: path.to_string(),
            existed_before,
            changed: before != after,
            unified: sanitize(&unified),
            added,
            removed,
        }
    }
}

/// Escrita atômica: grava em arquivo temporário vizinho e renomeia.
pub fn write_atomic(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.aimm-tmp-{}",
        target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        &uuid::Uuid::new_v4().to_string()[..8]
    ));
    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_and_restore_roundtrip_including_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path());
        let a = dir.path().join("cfg").join("a.json");
        let b = dir.path().join("cfg").join("b.json");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::write(&a, "{\"x\":1}").unwrap();
        let set = store.create("teste", Some("cursor"), &[a.clone(), b.clone()], vec![]).unwrap();
        assert!(set.files[0].existed && !set.files[1].existed);
        // alteração parcial simulando falha de instalação
        std::fs::write(&a, "{\"x\":2,\"ai-memory\":true}").unwrap();
        std::fs::write(&b, "novo").unwrap();
        let diffs = store.diff_against_current(&set);
        assert!(diffs[0].changed && diffs[1].changed);
        store.restore_files(&set).unwrap();
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "{\"x\":1}");
        assert!(!b.exists());
        assert_eq!(store.list().len(), 1);
        assert!(store.get(&set.id).is_some());
        assert!(store.get("../etc").is_none());
    }

    #[test]
    fn diff_counts_lines() {
        let d = FileDiff::new("x", "a\nb\n", "a\nc\n", true);
        assert_eq!((d.added, d.removed), (1, 1));
        assert!(d.unified.contains("-b") && d.unified.contains("+c"));
    }
}
