//! Log de operações locais (sanitizado) e catálogo/preferências da GUI.

use crate::sanitize::sanitize;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppOperation {
    pub id: String,
    pub action: String,
    pub kind: String, // "leitura" | "alteracao" | "teste"
    pub project_id: Option<String>,
    pub agent_id: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub success: Option<bool>,
    pub summary: Option<String>,
    pub backup_id: Option<String>,
}

pub struct AuditLog {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl AuditLog {
    pub fn new(app_dir: &Path) -> Self {
        let dir = app_dir.join("logs");
        let _ = std::fs::create_dir_all(&dir);
        Self { dir, lock: Mutex::new(()) }
    }

    fn index(&self) -> PathBuf {
        self.dir.join("operations.jsonl")
    }

    pub fn start(&self, action: &str, kind: &str, project_id: Option<&str>, agent_id: Option<&str>) -> AppOperation {
        AppOperation {
            id: uuid::Uuid::new_v4().to_string(),
            action: action.to_string(),
            kind: kind.to_string(),
            project_id: project_id.map(str::to_string),
            agent_id: agent_id.map(str::to_string),
            started_at: chrono::Local::now().to_rfc3339(),
            completed_at: None,
            success: None,
            summary: None,
            backup_id: None,
        }
    }

    /// Finaliza e persiste a operação com o log detalhado (sempre sanitizado).
    pub fn finish(&self, mut op: AppOperation, success: bool, summary: &str, detail: &str) -> AppOperation {
        op.completed_at = Some(chrono::Local::now().to_rfc3339());
        op.success = Some(success);
        op.summary = Some(sanitize(summary));
        let _g = self.lock.lock();
        let _ = std::fs::write(self.dir.join(format!("{}.log", op.id)), sanitize(detail));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.index()) {
            let _ = writeln!(f, "{}", serde_json::to_string(&op).unwrap_or_default());
        }
        op
    }

    pub fn list(&self, limit: usize) -> Vec<AppOperation> {
        let text = std::fs::read_to_string(self.index()).unwrap_or_default();
        let mut v: Vec<AppOperation> = text
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        v.reverse();
        v.truncate(limit);
        v
    }

    pub fn detail(&self, id: &str) -> Option<String> {
        if !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            return None;
        }
        std::fs::read_to_string(self.dir.join(format!("{id}.log"))).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_are_sanitized_and_listed_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let log = AuditLog::new(dir.path());
        let op = log.start("teste-1", "leitura", None, None);
        log.finish(op, true, "ok", "Authorization: Bearer abcdefghijklmnop123");
        let op2 = log.start("teste-2", "alteracao", Some("p"), Some("cursor"));
        let op2 = log.finish(op2, false, "falhou --auth-token segredo99999", "x");
        let list = log.list(10);
        assert_eq!(list[0].action, "teste-2");
        assert!(!list[0].summary.as_ref().unwrap().contains("segredo99999"));
        let first = &list[1];
        let d = log.detail(&first.id).unwrap();
        assert!(!d.contains("abcdefghijklmnop123"));
        assert!(log.detail("../x").is_none());
        assert_eq!(op2.success, Some(false));
        let raw = std::fs::read_to_string(dir.path().join("logs").join("operations.jsonl")).unwrap();
        assert!(!raw.contains("segredo99999"));
    }
}
