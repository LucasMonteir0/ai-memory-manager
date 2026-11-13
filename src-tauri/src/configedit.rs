//! Leitura e edição cirúrgica de configurações dos agentes.
//! Regra: só entradas de propriedade do ai-memory são tocadas; todo o resto é preservado.

use serde::Serialize;
use serde_json::{Map, Value};

pub const DEFAULT_MCP_NAME: &str = "ai-memory";

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("configuração JSON inválida: {0}")]
    InvalidJson(String),
    #[error("configuração TOML inválida: {0}")]
    InvalidToml(String),
    #[error("estrutura inesperada: {0}")]
    Unexpected(String),
}

pub fn parse_json(text: &str) -> Result<Value, EditError> {
    if text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(text).map_err(|e| EditError::InvalidJson(e.to_string()))
}

pub fn to_pretty_json(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s
}

/// Texto que identifica uma entrada de hook (command + args).
fn hook_text(h: &Value) -> String {
    let mut t = String::new();
    if let Some(c) = h.get("command").and_then(Value::as_str) {
        t.push_str(c);
    }
    if let Some(a) = h.get("args").and_then(Value::as_array) {
        for x in a {
            if let Some(s) = x.as_str() {
                t.push(' ');
                t.push_str(s);
            }
        }
    }
    if let Some(c) = h.get("bash").and_then(Value::as_str) {
        t.push(' ');
        t.push_str(c);
    }
    if let Some(c) = h.get("powershell").and_then(Value::as_str) {
        t.push(' ');
        t.push_str(c);
    }
    t.to_lowercase()
}

/// Um hook é do ai-memory se invoca o binário/scripts do ai-memory.
pub fn is_ai_memory_hook(h: &Value) -> bool {
    let t = hook_text(h);
    t.contains("ai-memory") && (t.contains(" hook") || t.contains("hooks\\") || t.contains("hooks/") || t.contains("ai-memory.exe"))
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookScan {
    /// eventos com pelo menos um hook ai-memory
    pub events: Vec<String>,
    /// eventos com mais de uma entrada ai-memory (possível duplicidade)
    pub duplicated_events: Vec<String>,
    /// hooks ai-memory que usam scripts (.ps1/.sh) em vez do comando nativo
    pub script_based: bool,
    /// existem hooks de terceiros (preservados)
    pub other_hooks: usize,
}

/// Percorre `root.hooks` aceitando os formatos agrupado (Claude/Codex: event -> [{matcher, hooks:[..]}])
/// e plano (Cursor: event -> [{command}]).
pub fn scan_hooks(root: &Value) -> HookScan {
    let mut scan = HookScan::default();
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return scan;
    };
    for (event, arr) in hooks {
        let Some(arr) = arr.as_array() else { continue };
        let mut n = 0;
        for entry in arr {
            let inner: Vec<&Value> = match entry.get("hooks").and_then(Value::as_array) {
                Some(list) => list.iter().collect(),
                None => vec![entry],
            };
            for h in inner {
                if is_ai_memory_hook(h) {
                    n += 1;
                    let t = hook_text(h);
                    if t.contains(".ps1") || t.contains(".sh") {
                        scan.script_based = true;
                    }
                } else {
                    scan.other_hooks += 1;
                }
            }
        }
        if n > 0 {
            scan.events.push(event.clone());
        }
        if n > 1 {
            scan.duplicated_events.push(event.clone());
        }
    }
    scan
}

/// Remove somente hooks do ai-memory. Retorna (novo valor, quantidade removida).
pub fn remove_ai_memory_hooks(mut root: Value) -> Result<(Value, usize), EditError> {
    let mut removed = 0;
    let Some(obj) = root.as_object_mut() else {
        return Err(EditError::Unexpected("raiz não é objeto".into()));
    };
    let Some(hooks) = obj.get_mut("hooks") else {
        return Ok((root, 0));
    };
    let Some(hooks) = hooks.as_object_mut() else {
        return Err(EditError::Unexpected("'hooks' não é objeto".into()));
    };
    let mut empty_events = vec![];
    for (event, arr) in hooks.iter_mut() {
        let Some(list) = arr.as_array_mut() else { continue };
        let mut new_list = Vec::with_capacity(list.len());
        for mut entry in list.drain(..) {
            if let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) {
                let before = inner.len();
                inner.retain(|h| !is_ai_memory_hook(h));
                removed += before - inner.len();
                if inner.is_empty() {
                    continue; // grupo ficou vazio
                }
                new_list.push(entry);
            } else if is_ai_memory_hook(&entry) {
                removed += 1;
            } else {
                new_list.push(entry);
            }
        }
        *list = new_list;
        if list.is_empty() {
            empty_events.push(event.clone());
        }
    }
    for e in empty_events {
        hooks.remove(&e);
    }
    Ok((root, removed))
}

/// Entrada MCP em JSON (`mcpServers.<name>`).
pub fn json_mcp_entry<'a>(root: &'a Value, name: &str) -> Option<&'a Value> {
    root.get("mcpServers").and_then(|m| m.get(name))
}

pub fn remove_json_mcp(mut root: Value, name: &str) -> Result<(Value, bool), EditError> {
    let removed = root
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .map(|m| m.remove(name).is_some())
        .unwrap_or(false);
    Ok((root, removed))
}

/// Outras entradas MCP preservadas (para exibir/validar RQ-09).
pub fn json_mcp_names(root: &Value) -> Vec<String> {
    root.get("mcpServers")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------- TOML (Codex)

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMcp {
    pub present: bool,
    pub url: Option<String>,
    pub command: Option<String>,
    pub bearer_token_env_var: Option<String>,
    pub has_literal_auth_header: bool,
    pub other_servers: Vec<String>,
    pub args: Vec<String>,
}

impl CodexMcp {
    /// Entrada usa a ponte stdio local do ai-memory (autentica sozinha pelo diretório de dados).
    pub fn uses_bridge(&self) -> bool {
        self.command.as_deref().map(|c| c.to_lowercase().contains("ai-memory")).unwrap_or(false)
            && self.args.iter().any(|a| a == "mcp-bridge")
    }
}

/// Converte `[mcp_servers.<name>]` do Codex para a ponte stdio `ai-memory.exe mcp-bridge`,
/// removendo url/token por variável e preservando as demais chaves (ex.: aprovações por ferramenta).
pub fn codex_mcp_to_bridge(text: &str, name: &str, exe: &str, mcp_url: &str) -> Result<String, EditError> {
    let mut doc: toml_edit::DocumentMut =
        text.parse().map_err(|e: toml_edit::TomlError| EditError::InvalidToml(e.to_string()))?;
    let servers = doc
        .get_mut("mcp_servers")
        .and_then(|i| i.as_table_like_mut())
        .ok_or_else(|| EditError::Unexpected("sem [mcp_servers]".into()))?;
    let entry = servers
        .get_mut(name)
        .and_then(|i| i.as_table_like_mut())
        .ok_or_else(|| EditError::Unexpected(format!("sem [mcp_servers.{name}]")))?;
    for k in ["url", "bearer_token_env_var", "http_headers", "headers", "env_http_headers"] {
        entry.remove(k);
    }
    entry.insert("command", toml_edit::value(exe));
    let mut args = toml_edit::Array::new();
    for a in ["mcp-bridge", "--no-session-id", "--server-url", mcp_url] {
        args.push(a);
    }
    entry.insert("args", toml_edit::value(args));
    Ok(doc.to_string())
}

pub fn codex_mcp(text: &str, name: &str) -> Result<CodexMcp, EditError> {
    let doc: toml_edit::DocumentMut =
        text.parse().map_err(|e: toml_edit::TomlError| EditError::InvalidToml(e.to_string()))?;
    let mut out = CodexMcp::default();
    if let Some(servers) = doc.get("mcp_servers").and_then(|i| i.as_table_like()) {
        for (k, v) in servers.iter() {
            if k == name {
                out.present = true;
                if let Some(t) = v.as_table_like() {
                    out.url = t.get("url").and_then(|x| x.as_str()).map(str::to_string);
                    out.command = t.get("command").and_then(|x| x.as_str()).map(str::to_string);
                    out.bearer_token_env_var = t
                        .get("bearer_token_env_var")
                        .and_then(|x| x.as_str())
                        .map(str::to_string);
                    out.has_literal_auth_header = t.get("http_headers").is_some()
                        || t.get("headers").is_some();
                    out.args = t
                        .get("args")
                        .and_then(|x| x.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                        .unwrap_or_default();
                }
            } else {
                out.other_servers.push(k.to_string());
            }
        }
    }
    Ok(out)
}

pub fn remove_codex_mcp(text: &str, name: &str) -> Result<(String, bool), EditError> {
    let mut doc: toml_edit::DocumentMut =
        text.parse().map_err(|e: toml_edit::TomlError| EditError::InvalidToml(e.to_string()))?;
    let removed = doc
        .get_mut("mcp_servers")
        .and_then(|i| i.as_table_like_mut())
        .map(|t| t.remove(name).is_some())
        .unwrap_or(false);
    Ok((doc.to_string(), removed))
}

/// Detecta plugin ai-memory no Codex (tabelas `[plugins."ai-memory@..."]`).
pub fn codex_plugins_mentioning_ai_memory(text: &str) -> Vec<String> {
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else { return vec![] };
    doc.get("plugins")
        .and_then(|i| i.as_table_like())
        .map(|t| {
            t.iter()
                .map(|(k, _)| k.to_string())
                .filter(|k| k.to_lowercase().contains("ai-memory"))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn claude_settings() -> Value {
        json!({
          "agentPushNotifEnabled": true,
          "hooks": {
            "SessionStart": [
              {"matcher": "", "hooks": [
                {"type": "command", "command": "C:\\Users\\u\\AppData\\Local\\Programs\\ai-memory\\ai-memory.exe",
                 "args": ["--data-dir", "C:\\x", "hook", "--event", "session-start", "--agent", "claude-code"]}
              ]},
              {"matcher": "", "hooks": [
                {"type": "command", "command": "node", "args": ["meu-hook.js"]}
              ]}
            ],
            "Stop": [
              {"matcher": "", "hooks": [
                {"type": "command", "command": "C:\\Users\\u\\AppData\\Local\\Programs\\ai-memory\\ai-memory.exe",
                 "args": ["hook", "--event", "stop", "--agent", "claude-code"]},
                {"type": "command", "command": "echo", "args": ["outro"]}
              ]}
            ]
          }
        })
    }

    #[test]
    fn scan_counts_ai_memory_and_others() {
        let s = scan_hooks(&claude_settings());
        assert_eq!(s.events.len(), 2);
        assert_eq!(s.other_hooks, 2);
        assert!(s.duplicated_events.is_empty());
        assert!(!s.script_based);
    }

    #[test]
    fn remove_hooks_preserves_third_party() {
        let (v, n) = remove_ai_memory_hooks(claude_settings()).unwrap();
        assert_eq!(n, 2);
        let txt = v.to_string();
        assert!(!txt.contains("ai-memory"));
        assert!(txt.contains("meu-hook.js") && txt.contains("outro"));
        assert_eq!(v["agentPushNotifEnabled"], json!(true));
        // grupo vazio de SessionStart removido, grupo de terceiros mantido
        assert_eq!(v["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn remove_hooks_flat_cursor_format() {
        let v = json!({"version": 1, "hooks": {
            "beforeSubmitPrompt": [
              {"command": "bash C:/Users/u/AppData/Local/Programs/ai-memory/hooks/cursor/before-submit.sh"},
              {"command": "./audit.sh"}
            ],
            "stop": [{"command": "bash C:/x/ai-memory/hooks/cursor/stop.sh"}]
        }});
        let s = scan_hooks(&v);
        assert!(s.script_based);
        let (v2, n) = remove_ai_memory_hooks(v).unwrap();
        assert_eq!(n, 2);
        assert!(v2["hooks"].get("stop").is_none());
        assert_eq!(v2["hooks"]["beforeSubmitPrompt"].as_array().unwrap().len(), 1);
        assert_eq!(v2["version"], json!(1));
    }

    #[test]
    fn duplicated_hooks_detected() {
        let v = json!({"hooks": {"Stop": [
            {"hooks": [{"command": "ai-memory.exe", "args": ["hook", "--event", "stop"]}]},
            {"hooks": [{"command": "ai-memory.exe", "args": ["hook", "--event", "stop"]}]}
        ]}});
        assert_eq!(scan_hooks(&v).duplicated_events, vec!["Stop".to_string()]);
    }

    #[test]
    fn remove_mcp_json_preserves_other_servers() {
        let v = json!({"mcpServers": {"ai-memory": {"command": "x"}, "outro": {"command": "y"}}, "numStartups": 5});
        let (v2, removed) = remove_json_mcp(v, "ai-memory").unwrap();
        assert!(removed);
        assert_eq!(json_mcp_names(&v2), vec!["outro".to_string()]);
        assert_eq!(v2["numStartups"], json!(5));
    }

    #[test]
    fn invalid_json_is_error() {
        assert!(parse_json("{ nope").is_err());
        assert!(parse_json("").unwrap().is_object());
    }

    const CODEX: &str = r#"model = "gpt"

[mcp_servers.node_repl]
command = 'C:\x\node_repl.exe'

[mcp_servers.ai-memory]
url = "http://127.0.0.1:49374/mcp"
bearer_token_env_var = "AI_MEMORY_AUTH_TOKEN"

[mcp_servers.ai-memory.tools.memory_delete_page]
approval_mode = "prompt"

# comentário preservado
[hooks.state.'C:\Users\u\.codex\hooks.json:stop:0:0']
trusted_hash = "abc"
"#;

    #[test]
    fn codex_mcp_detect_and_remove_preserves_rest() {
        let m = codex_mcp(CODEX, "ai-memory").unwrap();
        assert!(m.present);
        assert_eq!(m.bearer_token_env_var.as_deref(), Some("AI_MEMORY_AUTH_TOKEN"));
        assert_eq!(m.other_servers, vec!["node_repl".to_string()]);
        let (out, removed) = remove_codex_mcp(CODEX, "ai-memory").unwrap();
        assert!(removed);
        assert!(!out.contains("[mcp_servers.ai-memory"));
        assert!(out.contains("node_repl") && out.contains("# comentário preservado"));
        assert!(out.contains("trusted_hash"));
        assert!(out.contains("model = \"gpt\""));
    }

    #[test]
    fn codex_bridge_conversion_removes_env_token_and_keeps_rest() {
        let exe = r"C:\Users\u\AppData\Local\Programs\ai-memory\ai-memory.exe";
        let out = codex_mcp_to_bridge(CODEX, "ai-memory", exe, "http://127.0.0.1:49374/mcp").unwrap();
        let m = codex_mcp(&out, "ai-memory").unwrap();
        assert!(m.uses_bridge());
        assert_eq!(m.url, None);
        assert_eq!(m.bearer_token_env_var, None);
        assert_eq!(m.command.as_deref(), Some(exe));
        assert_eq!(m.args, vec!["mcp-bridge", "--no-session-id", "--server-url", "http://127.0.0.1:49374/mcp"]);
        // preservados: aprovação por ferramenta, outros servidores, trust de hooks, comentários
        assert!(out.contains("[mcp_servers.ai-memory.tools.memory_delete_page]"));
        assert!(out.contains("node_repl") && out.contains("trusted_hash") && out.contains("# comentário preservado"));
        // idempotente
        let again = codex_mcp_to_bridge(&out, "ai-memory", exe, "http://127.0.0.1:49374/mcp").unwrap();
        assert_eq!(again, out);
        assert!(codex_mcp_to_bridge("[mcp_servers.x]\n", "ai-memory", exe, "u").is_err());
    }

    #[test]
    fn codex_plugin_detection() {
        let t = "[plugins.\"ai-memory@marketplace\"]\nenabled = true\n[plugins.\"pdf@x\"]\n";
        assert_eq!(codex_plugins_mentioning_ai_memory(t).len(), 1);
    }

    #[test]
    fn invalid_toml_is_error() {
        assert!(codex_mcp("[[[", "ai-memory").is_err());
    }
}
