//! Detecção de aplicativos desktop e diagnóstico de MCP/hooks por agente.
//! MCP (consulta) e hooks (captura) são avaliados separadamente (RQ-05).

use crate::aimemory::Doctor;
use crate::configedit::{self, DEFAULT_MCP_NAME};
use crate::paths::SysPaths;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Status de integração (modelo de dados da especificação, seção 7).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStatus {
    NotDetected,
    NotConfigured,
    Configured,
    Verified,
    Untrusted,
    Partial,
    Error,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub status: IntegrationStatus,
    pub detail: String,
    pub config_path: Option<String>,
    pub source: Option<String>,
    pub evidence: Vec<String>,
    pub can_configure: bool,
    pub can_disable: bool,
    pub blocked_reason: Option<String>,
    pub experimental: Option<String>,
}

impl Component {
    fn unsupported(detail: &str) -> Self {
        Self {
            status: IntegrationStatus::Unsupported,
            detail: detail.into(),
            config_path: None,
            source: None,
            evidence: vec![],
            can_configure: false,
            can_disable: false,
            blocked_reason: Some(detail.into()),
            experimental: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// None = não verificável
    pub installed: Option<bool>,
    pub path: Option<String>,
    pub version: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentReport {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub mcp_client_id: Option<String>,
    pub hooks_agent_id: Option<String>,
    pub app: AppInfo,
    pub mcp: Component,
    pub hooks: Component,
    pub hooks_scope: String,
    pub notes: Vec<String>,
    pub trusted_events: Vec<String>,
    pub untrusted_events: Vec<String>,
    pub restart_hint: String,
    pub docs_url: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopApps {
    pub claude: AppInfo,
    pub codex: AppInfo,
    pub cursor: AppInfo,
    pub node_available: bool,
    pub git_bash: Option<String>,
}

// ---------------------------------------------------------------- detecção de apps

pub async fn detect_desktop_apps(paths: &SysPaths) -> DesktopApps {
    let mut out = DesktopApps::default();
    // MSIX: comando fixo, sem entrada do usuário.
    let script = "Get-AppxPackage | Where-Object { $_.Name -like 'Claude*' -or $_.Name -like '*Codex*' } | Select-Object Name,Version,InstallLocation | ConvertTo-Json -Compress";
    let spec = crate::runner::CmdSpec::new(
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    )
    .timeout(Duration::from_secs(25));
    let mut msix_checked = false;
    if let Ok(o) = crate::runner::run(&spec).await {
        if o.ok() {
            msix_checked = true;
            let v: Value = serde_json::from_str(o.stdout.trim()).unwrap_or(Value::Null);
            let items: Vec<Value> = match v {
                Value::Array(a) => a,
                Value::Object(_) => vec![v],
                _ => vec![],
            };
            for it in items {
                let name = it.get("Name").and_then(Value::as_str).unwrap_or("");
                let info = AppInfo {
                    installed: Some(true),
                    path: it.get("InstallLocation").and_then(Value::as_str).map(str::to_string),
                    version: it.get("Version").and_then(Value::as_str).map(str::to_string),
                    source: Some(format!("MSIX {name}")),
                };
                if name.to_lowercase().starts_with("claude") {
                    out.claude = info;
                } else if name.to_lowercase().contains("codex") {
                    out.codex = info;
                }
            }
        }
    }
    // Instalações clássicas
    if out.claude.installed != Some(true) {
        let classic = paths.localappdata.join("AnthropicClaude").join("claude.exe");
        if classic.is_file() {
            out.claude = AppInfo {
                installed: Some(true),
                path: Some(classic.to_string_lossy().to_string()),
                version: None,
                source: Some("instalador clássico".into()),
            };
        } else if msix_checked {
            out.claude.installed = Some(false);
        }
    }
    if out.codex.installed != Some(true) {
        out.codex.installed = if msix_checked { Some(false) } else { None };
    }
    let cursor = paths.localappdata.join("Programs").join("cursor").join("Cursor.exe");
    if cursor.is_file() {
        let pkg = paths
            .localappdata
            .join("Programs")
            .join("cursor")
            .join("resources")
            .join("app")
            .join("package.json");
        let version = std::fs::read_to_string(pkg)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("version").and_then(Value::as_str).map(str::to_string));
        out.cursor = AppInfo {
            installed: Some(true),
            path: Some(cursor.to_string_lossy().to_string()),
            version,
            source: Some("instalação por usuário".into()),
        };
    } else {
        out.cursor.installed = Some(paths.cursor_dir().exists());
        if out.cursor.installed == Some(true) {
            out.cursor.source = Some("apenas configuração ~/.cursor encontrada".into());
        }
    }
    out.node_available = which("node.exe").is_some();
    out.git_bash = find_git_bash();
    out
}

pub fn which(exe: &str) -> Option<PathBuf> {
    let pathvar = std::env::var_os("PATH")?;
    std::env::split_paths(&pathvar)
        .map(|d| d.join(exe))
        .find(|p| p.is_file())
}

/// Git Bash (não confundir com C:\Windows\System32\bash.exe, que é WSL).
pub fn find_git_bash() -> Option<String> {
    let mut cands = vec![
        PathBuf::from(r"C:\Program Files\Git\bin\bash.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Git\bin\bash.exe"),
    ];
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        cands.push(PathBuf::from(l).join("Programs").join("Git").join("bin").join("bash.exe"));
    }
    if let Some(g) = which("git.exe") {
        // ...\Git\cmd\git.exe -> ...\Git\bin\bash.exe
        if let Some(root) = g.parent().and_then(Path::parent) {
            cands.push(root.join("bin").join("bash.exe"));
        }
    }
    cands
        .into_iter()
        .find(|p| p.is_file() && !p.to_string_lossy().to_lowercase().contains("system32"))
        .map(|p| p.to_string_lossy().to_string())
}

#[cfg(windows)]
pub fn user_env_var(name: &str) -> Option<String> {
    crate::service::reg_get("Environment", name).or_else(|| std::env::var(name).ok()).filter(|s| !s.is_empty())
}
#[cfg(not(windows))]
pub fn user_env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------- arquivos por agente

/// Arquivos que podem ser alterados por install-mcp/install-hooks de cada agente.
pub fn agent_files(paths: &SysPaths, agent: &str, component: &str, claude_desktop_cfg: Option<&Path>) -> Vec<PathBuf> {
    match (agent, component) {
        ("claude-code", "mcp") => vec![paths.claude_json()],
        ("claude-code", "hooks") => vec![paths.claude_settings()],
        ("codex", "mcp") => vec![paths.codex_home().join("config.toml")],
        ("codex", "hooks") => vec![paths.codex_home().join("hooks.json"), paths.codex_home().join("config.toml")],
        ("cursor", "mcp") => vec![paths.cursor_dir().join("mcp.json")],
        ("cursor", "hooks") => vec![paths.cursor_dir().join("hooks.json")],
        ("claude-desktop", "mcp") => claude_desktop_cfg.map(|p| vec![p.to_path_buf()]).unwrap_or_default(),
        _ => vec![],
    }
}

/// Arquivo principal passado a `--config-file` (garante que a GUI sabe exatamente o que muda).
pub fn primary_config_file(paths: &SysPaths, agent: &str, component: &str, claude_desktop_cfg: Option<&Path>) -> Option<PathBuf> {
    agent_files(paths, agent, component, claude_desktop_cfg).into_iter().next()
}

/// Resolve o config do Claude Desktop (Chat), sem escolher às cegas quando ambíguo.
pub fn resolve_claude_desktop_config(paths: &SysPaths, apps: &DesktopApps) -> Result<PathBuf, String> {
    let cands = paths.claude_desktop_config_candidates();
    let existing_files: Vec<_> = cands.iter().filter(|(p, _)| p.is_file()).collect();
    if existing_files.len() == 1 {
        return Ok(existing_files[0].0.clone());
    }
    if existing_files.len() > 1 {
        return Err(format!(
            "Há {} arquivos claude_desktop_config.json; não é possível escolher automaticamente: {}",
            existing_files.len(),
            existing_files.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>().join("; ")
        ));
    }
    let is_msix = apps.claude.source.as_deref().map(|s| s.starts_with("MSIX")).unwrap_or(false);
    let msix: Vec<_> = cands
        .iter()
        .filter(|(p, s)| s.starts_with("msix:") && p.parent().and_then(Path::parent).map(|d| d.exists()).unwrap_or(false))
        .collect();
    if is_msix {
        return match msix.len() {
            1 => Ok(msix[0].0.clone()),
            0 => Err("Claude Desktop (MSIX) detectado, mas a pasta LocalCache\\Roaming não existe. Abra o Claude Desktop uma vez.".into()),
            _ => Err("Mais de um pacote MSIX do Claude encontrado; selecione manualmente.".into()),
        };
    }
    if apps.claude.installed == Some(true) {
        return Ok(cands[0].0.clone());
    }
    Err("Claude Desktop não detectado.".into())
}

// ---------------------------------------------------------------- diagnóstico

pub struct Ctx<'a> {
    pub paths: &'a SysPaths,
    pub apps: &'a DesktopApps,
    pub doctor: Option<&'a Doctor>,
    pub server_requires_token: bool,
}

fn read(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

fn json_file(p: &Path) -> Result<Option<Value>, String> {
    match read(p) {
        None => Ok(None),
        Some(t) => configedit::parse_json(&t).map(Some).map_err(|e| e.to_string()),
    }
}

const CLAUDE_CORE_EVENTS: &[&str] = &["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop", "SessionEnd"];
const CODEX_CORE_EVENTS: &[&str] = &["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop"];

fn hooks_component(path: &Path, core: &[&str], agent_label: &str) -> Component {
    let mut c = Component {
        status: IntegrationStatus::NotConfigured,
        detail: String::new(),
        config_path: Some(path.to_string_lossy().to_string()),
        source: Some("arquivo de hooks".into()),
        evidence: vec![],
        can_configure: true,
        can_disable: false,
        blocked_reason: None,
        experimental: None,
    };
    match json_file(path) {
        Err(e) => {
            c.status = IntegrationStatus::Error;
            c.detail = format!("Arquivo inválido: {e}");
            c.can_configure = false;
            c.blocked_reason = Some("Corrija o JSON antes; a GUI não altera arquivos inválidos.".into());
        }
        Ok(None) => c.detail = "Arquivo não existe; nenhum hook ai-memory.".into(),
        Ok(Some(v)) => {
            let s = configedit::scan_hooks(&v);
            if s.events.is_empty() {
                c.detail = "Nenhum hook ai-memory registrado.".into();
            } else {
                let missing: Vec<&str> = core
                    .iter()
                    .copied()
                    .filter(|e| !s.events.iter().any(|x| x.eq_ignore_ascii_case(e)))
                    .collect();
                c.status = if missing.is_empty() { IntegrationStatus::Configured } else { IntegrationStatus::Partial };
                c.detail = if missing.is_empty() {
                    format!("{} evento(s) de {agent_label} registrados.", s.events.len())
                } else {
                    format!("Registrados: {}. Faltando: {}.", s.events.join(", "), missing.join(", "))
                };
                c.evidence.push(format!("Eventos: {}", s.events.join(", ")));
                c.can_disable = true;
            }
            if !s.duplicated_events.is_empty() {
                c.evidence.push(format!("Possível duplicidade em: {}", s.duplicated_events.join(", ")));
                c.status = IntegrationStatus::Partial;
            }
            if s.script_based {
                c.evidence.push("Usa scripts (.ps1/.sh) em vez do comando nativo ai-memory.exe hook.".into());
            }
            if s.other_hooks > 0 {
                c.evidence.push(format!("{} hook(s) de terceiros serão preservados.", s.other_hooks));
            }
        }
    }
    c
}

fn json_mcp_component(path: &Path, label_src: &str) -> Component {
    let mut c = Component {
        status: IntegrationStatus::NotConfigured,
        detail: String::new(),
        config_path: Some(path.to_string_lossy().to_string()),
        source: Some(label_src.into()),
        evidence: vec![],
        can_configure: true,
        can_disable: false,
        blocked_reason: None,
        experimental: None,
    };
    match json_file(path) {
        Err(e) => {
            c.status = IntegrationStatus::Error;
            c.detail = format!("Arquivo inválido: {e}");
            c.can_configure = false;
            c.blocked_reason = Some("Corrija o JSON antes; a GUI não altera arquivos inválidos.".into());
        }
        Ok(None) => c.detail = "Arquivo não existe; MCP ai-memory ausente.".into(),
        Ok(Some(v)) => {
            let others: Vec<String> = configedit::json_mcp_names(&v).into_iter().filter(|n| n != DEFAULT_MCP_NAME).collect();
            if let Some(e) = configedit::json_mcp_entry(&v, DEFAULT_MCP_NAME) {
                c.status = IntegrationStatus::Configured;
                let cmd = e.get("command").and_then(Value::as_str).unwrap_or("");
                let url = e.get("url").and_then(Value::as_str).unwrap_or("");
                let args = e.get("args").map(|a| a.to_string()).unwrap_or_default();
                c.detail = if !url.is_empty() {
                    format!("Entrada '{DEFAULT_MCP_NAME}' via HTTP ({url}).")
                } else if args.contains("mcp-bridge") {
                    "Entrada 'ai-memory' via ponte stdio nativa (ai-memory.exe mcp-bridge).".into()
                } else if cmd.contains("npx") {
                    "Entrada 'ai-memory' via npx (requer Node.js).".into()
                } else {
                    "Entrada 'ai-memory' presente.".into()
                };
                c.evidence.push("Configuração encontrada (não prova que o app carregou).".into());
                c.can_disable = true;
            } else {
                c.detail = "Sem entrada 'ai-memory'.".into();
            }
            if !others.is_empty() {
                c.evidence.push(format!("Outros servidores MCP preservados: {}", others.join(", ")));
            }
        }
    }
    c
}

pub fn claude_code_report(ctx: &Ctx) -> AgentReport {
    let p = ctx.paths;
    let mut mcp = json_mcp_component(&p.claude_json(), "~/.claude.json (MCP manual)");
    let mut hooks = hooks_component(&p.claude_settings(), CLAUDE_CORE_EVENTS, "Claude Code");
    let mut notes = vec![
        "A aba Code do Claude Desktop usa a engine do Claude Code: hooks/MCP vêm de ~/.claude, não do claude_desktop_config.json.".to_string(),
        "Não é necessário o comando `claude` no PATH.".to_string(),
    ];
    // plugin
    if let Ok(Some(v)) = json_file(&p.claude_settings()) {
        if let Some(pl) = v.get("enabledPlugins").and_then(Value::as_object) {
            let ai: Vec<_> = pl.keys().filter(|k| k.to_lowercase().contains("ai-memory")).cloned().collect();
            if !ai.is_empty() {
                notes.push(format!("Plugin ai-memory habilitado: {}. Evite instalar hooks/MCP standalone duplicados.", ai.join(", ")));
                if hooks.status != IntegrationStatus::NotConfigured {
                    hooks.evidence.push("Plugin + hooks standalone: risco de captura duplicada.".into());
                    hooks.status = IntegrationStatus::Partial;
                }
                hooks.source = Some("plugin".into());
                mcp.source = Some("plugin".into());
            }
        }
    }
    if let Some(d) = ctx.doctor {
        if let Some(r) = d.rows.iter().find(|r| r.agent == "claude-code") {
            hooks.evidence.push(format!(
                "doctor {}/{}: {} sessão(ões) locais, {} capturada(s).",
                d.workspace, d.project, r.local_total, r.captured
            ));
        }
    }
    let installed = ctx.apps.claude.installed.or(Some(p.claude_dir().exists()));
    AgentReport {
        id: "claude-code".into(),
        display_name: "Claude Desktop — Code".into(),
        description: "Aba Code do Claude Desktop (engine Claude Code).".into(),
        mcp_client_id: Some("claude-code".into()),
        hooks_agent_id: Some("claude-code".into()),
        app: AppInfo { installed, ..ctx.apps.claude.clone() },
        mcp,
        hooks,
        hooks_scope: "Global (~/.claude/settings.json). Escopo por projeto existe (settings.local.json) mas não é usado pela GUI.".into(),
        notes,
        trusted_events: vec![],
        untrusted_events: vec![],
        restart_hint: "Feche e reabra o Claude Desktop (ou inicie uma nova sessão na aba Code) para carregar as mudanças.".into(),
        docs_url: "https://github.com/akitaonrails/ai-memory/blob/main/docs/mcp-install.md".into(),
    }
}

pub fn codex_report(ctx: &Ctx) -> AgentReport {
    let p = ctx.paths;
    let cfg_path = p.codex_home().join("config.toml");
    let cfg_text = read(&cfg_path);
    let mut notes = vec!["Não depende do comando `codex`; edita apenas a configuração em CODEX_HOME.".to_string()];
    let mut mcp = Component {
        status: IntegrationStatus::NotConfigured,
        detail: String::new(),
        config_path: Some(cfg_path.to_string_lossy().to_string()),
        source: Some("config.toml".into()),
        evidence: vec![],
        can_configure: true,
        can_disable: false,
        blocked_reason: None,
        experimental: None,
    };
    let mut plugins = vec![];
    match cfg_text.as_deref().map(|t| configedit::codex_mcp(t, DEFAULT_MCP_NAME)) {
        None => mcp.detail = "config.toml não existe.".into(),
        Some(Err(e)) => {
            mcp.status = IntegrationStatus::Error;
            mcp.detail = e.to_string();
            mcp.can_configure = false;
            mcp.blocked_reason = Some("Corrija o TOML antes.".into());
        }
        Some(Ok(m)) => {
            plugins = configedit::codex_plugins_mentioning_ai_memory(cfg_text.as_deref().unwrap_or(""));
            if m.present {
                mcp.status = IntegrationStatus::Configured;
                mcp.can_disable = true;
                mcp.detail = if m.uses_bridge() {
                    "[mcp_servers.ai-memory] via ponte stdio local (ai-memory.exe mcp-bridge); o token é lido do diretório de dados.".into()
                } else {
                    format!("[mcp_servers.ai-memory] {}", m.url.clone().or(m.command.clone()).unwrap_or_default())
                };
                if let Some(var) = &m.bearer_token_env_var {
                    if user_env_var(var).is_none() && ctx.server_requires_token {
                        mcp.status = IntegrationStatus::Error;
                        mcp.detail = format!(
                            "O token é lido da variável {var}, que não existe no ambiente do usuário. O Codex provavelmente recebe 401 ao chamar o MCP."
                        );
                        mcp.evidence.push("Correção: veja 'Corrigir autenticação' (requer confirmação explícita).".into());
                    } else {
                        mcp.evidence.push(format!("Token via variável {var} (presente)."));
                    }
                }
                if !m.other_servers.is_empty() {
                    mcp.evidence.push(format!("Outros servidores preservados: {}", m.other_servers.join(", ")));
                }
            } else {
                mcp.detail = "Sem [mcp_servers.ai-memory].".into();
            }
        }
    }
    let mut hooks = hooks_component(&p.codex_home().join("hooks.json"), CODEX_CORE_EVENTS, "Codex");
    let (mut trusted, mut untrusted) = (vec![], vec![]);
    if let Some(d) = ctx.doctor {
        trusted = d.codex_hooks_trusted.clone();
        untrusted = d.codex_hooks_untrusted.clone();
        plugins.extend(d.plugins.iter().filter(|x| x.to_lowercase().contains("ai-memory")).cloned());
        if let Some(r) = d.rows.iter().find(|r| r.agent == "codex") {
            hooks.evidence.push(format!(
                "doctor {}/{}: {} sessão(ões) locais, {} capturada(s).",
                d.workspace, d.project, r.local_total, r.captured
            ));
        }
    } else if let Some(t) = &cfg_text {
        // fallback: contar entradas [hooks.state] com trusted_hash
        let n = t.matches("trusted_hash").count();
        if n > 0 {
            hooks.evidence.push(format!("{n} entrada(s) de trust em [hooks.state] (rode o doctor para detalhar)."));
        }
    }
    if !untrusted.is_empty() && hooks.status != IntegrationStatus::NotConfigured {
        hooks.status = IntegrationStatus::Untrusted;
        hooks.detail = format!(
            "Hooks não autorizados: {}. Ação manual: abra o Codex Desktop e aprove os hooks quando solicitado. A GUI não grava trust.",
            untrusted.join(", ")
        );
    } else if !trusted.is_empty() {
        hooks.evidence.push(format!("Autorizados no Codex: {}", trusted.join(", ")));
    }
    if !plugins.is_empty() {
        notes.push(format!("Plugin ai-memory detectado ({}). Instalação standalone bloqueada para evitar duplicidade.", plugins.join(", ")));
        for c in [&mut mcp, &mut hooks] {
            c.source = Some("plugin".into());
            c.can_configure = false;
            c.blocked_reason = Some("Integração via plugin já existe.".into());
        }
    }
    notes.push("Ao reinstalar hooks, o hash muda e o Codex volta a pedir aprovação.".into());
    AgentReport {
        id: "codex".into(),
        display_name: "Codex Desktop".into(),
        description: "OpenAI Codex (aplicativo desktop).".into(),
        mcp_client_id: Some("codex".into()),
        hooks_agent_id: Some("codex".into()),
        app: AppInfo {
            installed: ctx.apps.codex.installed.or(Some(p.codex_home().exists())),
            ..ctx.apps.codex.clone()
        },
        mcp,
        hooks,
        hooks_scope: "Global (CODEX_HOME). Não há escopo por projeto.".into(),
        notes,
        trusted_events: trusted,
        untrusted_events: untrusted,
        restart_hint: "Feche e reabra o Codex Desktop; se pedir aprovação de hooks, aprove na própria interface.".into(),
        docs_url: "https://github.com/akitaonrails/ai-memory/blob/main/docs/mcp-install.md".into(),
    }
}

pub fn cursor_report(ctx: &Ctx) -> AgentReport {
    let p = ctx.paths;
    let mcp = json_mcp_component(&p.cursor_dir().join("mcp.json"), "~/.cursor/mcp.json");
    let mut hooks = hooks_component(&p.cursor_dir().join("hooks.json"), &["beforeSubmitPrompt", "stop"], "Cursor");
    hooks.experimental = Some(
        "Experimental no Windows nativo: o ai-memory 3.4.6 gera hooks .sh para o Cursor, que exigem Git Bash e ainda não têm validação oficial.".into(),
    );
    if ctx.apps.git_bash.is_none() {
        hooks.can_configure = false;
        hooks.blocked_reason = Some("Git Bash não encontrado; hooks do Cursor indisponíveis neste Windows.".into());
        if hooks.status == IntegrationStatus::NotConfigured {
            hooks.status = IntegrationStatus::Unsupported;
        }
    } else {
        hooks.evidence.push(format!("Git Bash: {}", ctx.apps.git_bash.clone().unwrap_or_default()));
    }
    if let Some(d) = ctx.doctor {
        if let Some(r) = d.rows.iter().find(|r| r.agent == "cursor") {
            hooks.evidence.push(format!("doctor: {} locais, {} capturadas.", r.local_total, r.captured));
        }
    }
    let installed = ctx.apps.cursor.installed;
    let (mut mcp, mut hooks) = (mcp, hooks);
    if installed == Some(false) {
        for c in [&mut mcp, &mut hooks] {
            if c.status == IntegrationStatus::NotConfigured {
                c.status = IntegrationStatus::NotDetected;
            }
        }
    }
    AgentReport {
        id: "cursor".into(),
        display_name: "Cursor".into(),
        description: "Cursor Desktop (agente).".into(),
        mcp_client_id: Some("cursor".into()),
        hooks_agent_id: Some("cursor".into()),
        app: ctx.apps.cursor.clone(),
        mcp,
        hooks,
        hooks_scope: "Global (~/.cursor). Escopo de projeto (.cursor/) não é gerenciado pela GUI.".into(),
        notes: vec!["Após configurar o MCP, reinicie o Cursor ou alterne o servidor em Settings → MCP.".into()],
        trusted_events: vec![],
        untrusted_events: vec![],
        restart_hint: "Reinicie o Cursor (ou desligue/ligue o servidor em Settings → MCP).".into(),
        docs_url: "https://github.com/akitaonrails/ai-memory/blob/main/docs/mcp-install.md".into(),
    }
}

pub fn claude_desktop_report(ctx: &Ctx) -> AgentReport {
    let resolved = resolve_claude_desktop_config(ctx.paths, ctx.apps);
    let mut mcp = match &resolved {
        Ok(p) => json_mcp_component(p, "claude_desktop_config.json"),
        Err(e) => {
            let mut c = Component::unsupported(e);
            c.status = if ctx.apps.claude.installed == Some(true) { IntegrationStatus::Error } else { IntegrationStatus::NotDetected };
            c
        }
    };
    let mut notes = vec![
        "Somente MCP: a aba Chat não tem hooks; não há captura automática de conversas.".to_string(),
        "Após alterar, feche totalmente e reabra o Claude Desktop (\"Check for Updates\" não basta).".to_string(),
    ];
    if let Ok(Some(v)) = resolved.as_ref().map(|p| json_file(p)).unwrap_or(Ok(None)) {
        if let Some(e) = configedit::json_mcp_entry(&v, DEFAULT_MCP_NAME) {
            if e.get("command").and_then(Value::as_str).map(|c| c.contains("npx")).unwrap_or(false) && !ctx.apps.node_available {
                mcp.status = IntegrationStatus::Error;
                mcp.detail = "A entrada usa npx/mcp-remote, mas o Node.js não foi encontrado. Instale o Node.js LTS manualmente.".into();
            }
        }
    }
    if ctx.apps.claude.installed != Some(true) {
        notes.push("Claude Desktop não detectado.".into());
    }
    AgentReport {
        id: "claude-desktop".into(),
        display_name: "Claude Desktop — Chat".into(),
        description: "Aba Chat do Claude Desktop (somente MCP).".into(),
        mcp_client_id: Some("claude-desktop".into()),
        hooks_agent_id: None,
        app: ctx.apps.claude.clone(),
        mcp,
        hooks: Component::unsupported("Somente MCP — a aba Chat não oferece hooks de captura."),
        hooks_scope: "—".into(),
        notes,
        trusted_events: vec![],
        untrusted_events: vec![],
        restart_hint: "Feche totalmente (bandeja) e reabra o Claude Desktop.".into(),
        docs_url: "https://github.com/akitaonrails/ai-memory/blob/main/docs/mcp-install.md".into(),
    }
}

pub fn cowork_report(ctx: &Ctx) -> AgentReport {
    AgentReport {
        id: "claude-cowork".into(),
        display_name: "Claude Desktop — Cowork".into(),
        description: "Suporte específico não documentado pelo ai-memory.".into(),
        mcp_client_id: None,
        hooks_agent_id: None,
        app: ctx.apps.claude.clone(),
        mcp: Component::unsupported("Não verificado: o ai-memory 3.4.6 não documenta integração com o Cowork."),
        hooks: Component::unsupported("Não verificado."),
        hooks_scope: "—".into(),
        notes: vec!["Não exibido como compatível até haver suporte documentado.".into()],
        trusted_events: vec![],
        untrusted_events: vec![],
        restart_hint: String::new(),
        docs_url: "https://github.com/akitaonrails/ai-memory/blob/main/docs/support-matrix.md".into(),
    }
}

pub fn all_reports(ctx: &Ctx) -> Vec<AgentReport> {
    vec![claude_code_report(ctx), codex_report(ctx), cursor_report(ctx), claude_desktop_report(ctx), cowork_report(ctx)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aimemory::{Interpreted, parse_doctor};

    fn sandbox() -> (tempfile::TempDir, SysPaths) {
        let d = tempfile::tempdir().unwrap();
        let p = SysPaths::sandbox(d.path());
        std::fs::create_dir_all(&p.home).unwrap();
        (d, p)
    }

    fn apps_all() -> DesktopApps {
        DesktopApps {
            claude: AppInfo { installed: Some(true), source: Some("MSIX Claude".into()), ..Default::default() },
            codex: AppInfo { installed: Some(true), ..Default::default() },
            cursor: AppInfo { installed: Some(true), ..Default::default() },
            node_available: false,
            git_bash: None,
        }
    }

    #[test]
    fn claude_code_and_desktop_are_separate() {
        let (_d, p) = sandbox();
        std::fs::create_dir_all(p.claude_dir()).unwrap();
        std::fs::write(p.claude_settings(), r#"{"hooks":{"SessionStart":[{"hooks":[{"command":"C:\\x\\ai-memory.exe","args":["hook","--event","session-start"]}]}]}}"#).unwrap();
        std::fs::write(p.claude_json(), r#"{"mcpServers":{"ai-memory":{"type":"stdio","command":"ai-memory.exe","args":["mcp-bridge"]}}}"#).unwrap();
        let apps = apps_all();
        let ctx = Ctx { paths: &p, apps: &apps, doctor: None, server_requires_token: true };
        let code = claude_code_report(&ctx);
        assert_eq!(code.hooks.status, IntegrationStatus::Partial); // só SessionStart
        assert_eq!(code.mcp.status, IntegrationStatus::Configured);
        let chat = claude_desktop_report(&ctx);
        assert_eq!(chat.hooks.status, IntegrationStatus::Unsupported);
        assert!(chat.hooks_agent_id.is_none());
        assert!(chat.notes.iter().any(|n| n.contains("Somente MCP")));
    }

    #[test]
    fn codex_untrusted_from_doctor_and_auth_gap() {
        let (_d, p) = sandbox();
        std::fs::create_dir_all(p.codex_home()).unwrap();
        std::fs::write(
            p.codex_home().join("config.toml"),
            "[mcp_servers.ai-memory]\nurl = \"http://127.0.0.1:49374/mcp\"\nbearer_token_env_var = \"AIMM_TEST_VAR_THAT_DOES_NOT_EXIST\"\n",
        )
        .unwrap();
        std::fs::write(
            p.codex_home().join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"& 'C:\\x\\ai-memory.exe' hook --event session-start --agent codex"}]}],"Stop":[{"hooks":[{"command":"ai-memory.exe hook --event stop"}]}]}}"#,
        )
        .unwrap();
        let fx = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/doctor-projeto-b-untrusted.json")).unwrap();
        let Interpreted::Ok { data: doc } = parse_doctor(&fx) else { panic!() };
        let apps = apps_all();
        let ctx = Ctx { paths: &p, apps: &apps, doctor: Some(&doc), server_requires_token: true };
        let r = codex_report(&ctx);
        assert_eq!(r.hooks.status, IntegrationStatus::Untrusted);
        assert!(r.hooks.detail.contains("Ação manual"));
        assert_eq!(r.mcp.status, IntegrationStatus::Error);
        assert!(r.mcp.detail.contains("AIMM_TEST_VAR_THAT_DOES_NOT_EXIST"));
    }

    #[test]
    fn codex_plugin_blocks_standalone_install() {
        let (_d, p) = sandbox();
        std::fs::create_dir_all(p.codex_home()).unwrap();
        std::fs::write(p.codex_home().join("config.toml"), "[plugins.\"ai-memory@mkt\"]\nenabled = true\n").unwrap();
        let apps = apps_all();
        let ctx = Ctx { paths: &p, apps: &apps, doctor: None, server_requires_token: true };
        let r = codex_report(&ctx);
        assert!(!r.hooks.can_configure && !r.mcp.can_configure);
        assert_eq!(r.hooks.source.as_deref(), Some("plugin"));
    }

    #[test]
    fn cursor_hooks_blocked_without_git_bash_and_marked_experimental() {
        let (_d, p) = sandbox();
        let apps = apps_all();
        let ctx = Ctx { paths: &p, apps: &apps, doctor: None, server_requires_token: true };
        let r = cursor_report(&ctx);
        assert!(r.hooks.experimental.is_some());
        assert!(!r.hooks.can_configure);
        assert!(r.mcp.can_configure);
        assert_eq!(r.mcp.status, IntegrationStatus::NotConfigured);
    }

    #[test]
    fn invalid_json_blocks_changes() {
        let (_d, p) = sandbox();
        std::fs::create_dir_all(p.cursor_dir()).unwrap();
        std::fs::write(p.cursor_dir().join("mcp.json"), "{ quebrado").unwrap();
        let apps = apps_all();
        let ctx = Ctx { paths: &p, apps: &apps, doctor: None, server_requires_token: true };
        let r = cursor_report(&ctx);
        assert_eq!(r.mcp.status, IntegrationStatus::Error);
        assert!(!r.mcp.can_configure);
    }

    #[test]
    fn claude_desktop_ambiguous_msix_is_not_guessed() {
        let (_d, p) = sandbox();
        for id in ["Claude_aaa", "Claude_bbb"] {
            std::fs::create_dir_all(p.localappdata.join("Packages").join(id).join("LocalCache").join("Roaming")).unwrap();
        }
        let apps = apps_all();
        assert!(resolve_claude_desktop_config(&p, &apps).is_err());
    }

    #[test]
    fn cowork_is_never_compatible() {
        let (_d, p) = sandbox();
        let apps = apps_all();
        let ctx = Ctx { paths: &p, apps: &apps, doctor: None, server_requires_token: true };
        let r = cowork_report(&ctx);
        assert!(!r.mcp.can_configure && !r.hooks.can_configure);
    }
}
