//! Resolução dinâmica de caminhos do sistema e dos agentes (nada hardcoded por usuário).

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SysPaths {
    pub home: PathBuf,
    pub appdata: PathBuf,
    pub localappdata: PathBuf,
    pub claude_config_dir_env: Option<PathBuf>,
    pub codex_home_env: Option<PathBuf>,
    pub ai_memory_data_dir_env: Option<PathBuf>,
}

fn env_path(k: &str) -> Option<PathBuf> {
    std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from)
}

impl SysPaths {
    pub fn from_env() -> Self {
        let home = env_path("USERPROFILE")
            .or_else(|| env_path("HOME"))
            .unwrap_or_else(|| PathBuf::from("."));
        let appdata = env_path("APPDATA").unwrap_or_else(|| home.join("AppData").join("Roaming"));
        let localappdata =
            env_path("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData").join("Local"));
        Self {
            home,
            appdata,
            localappdata,
            claude_config_dir_env: env_path("CLAUDE_CONFIG_DIR"),
            codex_home_env: env_path("CODEX_HOME"),
            ai_memory_data_dir_env: env_path("AI_MEMORY_DATA_DIR"),
        }
    }

    /// Construtor para testes/sandbox.
    pub fn sandbox(root: &Path) -> Self {
        Self {
            home: root.join("home"),
            appdata: root.join("home").join("AppData").join("Roaming"),
            localappdata: root.join("home").join("AppData").join("Local"),
            claude_config_dir_env: None,
            codex_home_env: None,
            ai_memory_data_dir_env: None,
        }
    }

    pub fn app_dir(&self) -> PathBuf {
        self.appdata.join("AI Memory Manager")
    }

    pub fn ai_memory_default_data_dir(&self) -> PathBuf {
        self.ai_memory_data_dir_env
            .clone()
            .unwrap_or_else(|| self.localappdata.join("ai-memory"))
    }

    pub fn ai_memory_default_install_dir(&self) -> PathBuf {
        self.localappdata.join("Programs").join("ai-memory")
    }

    pub fn claude_dir(&self) -> PathBuf {
        self.claude_config_dir_env
            .clone()
            .unwrap_or_else(|| self.home.join(".claude"))
    }

    pub fn claude_settings(&self) -> PathBuf {
        self.claude_dir().join("settings.json")
    }

    /// MCP do Claude Code: `~/.claude.json` (ou `$CLAUDE_CONFIG_DIR/.claude.json`).
    pub fn claude_json(&self) -> PathBuf {
        match &self.claude_config_dir_env {
            Some(d) => d.join(".claude.json"),
            None => self.home.join(".claude.json"),
        }
    }

    pub fn codex_home(&self) -> PathBuf {
        self.codex_home_env
            .clone()
            .unwrap_or_else(|| self.home.join(".codex"))
    }

    pub fn cursor_dir(&self) -> PathBuf {
        self.home.join(".cursor")
    }

    /// Candidatos de configuração do Claude Desktop (Chat). Retorna (caminho, origem).
    pub fn claude_desktop_config_candidates(&self) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        out.push((
            self.appdata.join("Claude").join("claude_desktop_config.json"),
            "desktop-classico".to_string(),
        ));
        let pkgs = self.localappdata.join("Packages");
        if let Ok(rd) = std::fs::read_dir(&pkgs) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("Claude_") {
                    out.push((
                        e.path()
                            .join("LocalCache")
                            .join("Roaming")
                            .join("Claude")
                            .join("claude_desktop_config.json"),
                        format!("msix:{name}"),
                    ));
                }
            }
        }
        out
    }
}

/// Remove o prefixo `\\?\` produzido por `canonicalize` no Windows.
pub fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}

pub fn display(p: &Path) -> String {
    strip_verbatim(p).to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_verbatim_prefix() {
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\C:\Users\X")),
            PathBuf::from(r"C:\Users\X")
        );
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\UNC\srv\share\a")),
            PathBuf::from(r"\\srv\share\a")
        );
    }

    #[test]
    fn sandbox_paths_are_under_root() {
        let p = SysPaths::sandbox(Path::new("C:/tmp/x"));
        assert!(p.claude_settings().starts_with("C:/tmp/x"));
        assert!(p.codex_home().ends_with(".codex"));
    }
}
