//! Servidor global: saúde, token (somente em memória), teste MCP, iniciar/parar via
//! mecanismo nativo do `service-host` e autostart pela chave Run do HKCU.

use crate::sanitize::{register_secret, sanitize};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const RUN_VALUE: &str = "ai-memory";

// ---------------------------------------------------------------- config do ai-memory

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiMemoryConfig {
    pub config_path: String,
    pub exists: bool,
    pub bind: Option<String>,
    pub server_url: Option<String>,
    pub bearer_token_file: Option<String>,
    pub parse_error: Option<String>,
}

pub fn read_ai_memory_config(data_dir: &Path) -> AiMemoryConfig {
    let p = data_dir.join("config.toml");
    let mut c = AiMemoryConfig {
        config_path: p.to_string_lossy().to_string(),
        ..Default::default()
    };
    let Ok(text) = std::fs::read_to_string(&p) else { return c };
    c.exists = true;
    match text.parse::<toml::Table>() {
        Ok(t) => {
            c.bind = t.get("bind").and_then(|v| v.as_str()).map(str::to_string);
            c.server_url = t.get("server_url").and_then(|v| v.as_str()).map(str::to_string);
            c.bearer_token_file = t
                .get("auth")
                .and_then(|a| a.get("bearer_token_file"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        Err(e) => c.parse_error = Some(e.to_string()),
    }
    c
}

pub fn server_base_url(cfg: &AiMemoryConfig) -> String {
    if let Ok(u) = std::env::var("AI_MEMORY_SERVER_URL") {
        if !u.is_empty() {
            return u.trim_end_matches('/').to_string();
        }
    }
    if let Some(u) = &cfg.server_url {
        return u.trim_end_matches('/').to_string();
    }
    let bind = cfg.bind.clone().unwrap_or_else(|| "127.0.0.1:49374".into());
    let bind = bind.replace("0.0.0.0", "127.0.0.1");
    format!("http://{bind}")
}

/// Lê o token bearer apenas para uso interno do backend. Nunca é enviado ao frontend.
pub fn read_token(data_dir: &Path, cfg: &AiMemoryConfig) -> Option<String> {
    let mut candidates: Vec<PathBuf> = vec![];
    if let Some(f) = &cfg.bearer_token_file {
        let p = PathBuf::from(f);
        candidates.push(if p.is_absolute() { p } else { data_dir.join(p) });
    }
    candidates.push(data_dir.join("auth-token"));
    for p in candidates {
        if let Ok(t) = std::fs::read_to_string(&p) {
            let t = t.trim().to_string();
            if !t.is_empty() {
                register_secret(&t);
                return Some(t);
            }
        }
    }
    if let Ok(t) = std::env::var("AI_MEMORY_AUTH_TOKEN") {
        if !t.trim().is_empty() {
            register_secret(t.trim());
            return Some(t.trim().to_string());
        }
    }
    None
}

// ---------------------------------------------------------------- saúde

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub url: String,
    pub reachable: bool,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u128>,
    pub error: Option<String>,
    pub checked_at: String,
}

fn http_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .no_proxy()
        .user_agent(concat!("ai-memory-manager/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("client")
}

pub async fn health(base: &str) -> Health {
    let url = format!("{base}/healthz");
    let started = Instant::now();
    let r = http_client(Duration::from_secs(4)).get(&url).send().await;
    let now = chrono::Local::now().to_rfc3339();
    match r {
        Ok(resp) => Health {
            url,
            reachable: true,
            http_status: Some(resp.status().as_u16()),
            latency_ms: Some(started.elapsed().as_millis()),
            error: None,
            checked_at: now,
        },
        Err(e) => Health {
            url,
            reachable: false,
            http_status: None,
            latency_ms: None,
            error: Some(sanitize(&e.to_string())),
            checked_at: now,
        },
    }
}

// ---------------------------------------------------------------- teste MCP

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTest {
    /// nível da evidência: "servidor" (a GUI falou com o endpoint)
    pub level: &'static str,
    pub endpoint: String,
    pub with_token: bool,
    pub ok: bool,
    pub http_status: Option<u16>,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub tool_count: Option<usize>,
    pub tool_names: Vec<String>,
    pub error: Option<String>,
    pub tested_at: String,
}

fn parse_mcp_body(body: &str) -> Option<Value> {
    let t = body.trim_start();
    if t.starts_with('{') {
        return serde_json::from_str(t).ok();
    }
    // SSE: pega o último "data:" com JSON
    body.lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
        .last()
}

pub async fn test_mcp(base: &str, token: Option<&str>) -> McpTest {
    let endpoint = format!("{base}/mcp");
    let mut res = McpTest {
        level: "servidor",
        endpoint: endpoint.clone(),
        with_token: token.is_some(),
        ok: false,
        http_status: None,
        server_name: None,
        server_version: None,
        tool_count: None,
        tool_names: vec![],
        error: None,
        tested_at: chrono::Local::now().to_rfc3339(),
    };
    let client = http_client(Duration::from_secs(10));
    let post = |body: Value, session: Option<String>| {
        let mut rq = client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&body);
        if let Some(t) = token {
            rq = rq.bearer_auth(t);
        }
        if let Some(s) = session {
            rq = rq.header("Mcp-Session-Id", s);
        }
        rq.send()
    };
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "ai-memory-manager", "version": env!("CARGO_PKG_VERSION")}
        }
    });
    let resp = match post(init, None).await {
        Ok(r) => r,
        Err(e) => {
            res.error = Some(sanitize(&e.to_string()));
            return res;
        }
    };
    res.http_status = Some(resp.status().as_u16());
    let session = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if !resp.status().is_success() {
        res.error = Some(match resp.status().as_u16() {
            401 | 403 => "Autenticação recusada pelo servidor (token ausente ou inválido).".into(),
            s => format!("HTTP {s} no initialize"),
        });
        return res;
    }
    let body = resp.text().await.unwrap_or_default();
    let Some(v) = parse_mcp_body(&body) else {
        res.error = Some("Resposta do initialize não interpretada.".into());
        return res;
    };
    if let Some(err) = v.get("error") {
        res.error = Some(sanitize(&err.to_string()));
        return res;
    }
    res.server_name = v.pointer("/result/serverInfo/name").and_then(Value::as_str).map(str::to_string);
    res.server_version = v.pointer("/result/serverInfo/version").and_then(Value::as_str).map(str::to_string);
    let _ = post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}), session.clone()).await;
    match post(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}), session).await {
        Ok(r) if r.status().is_success() => {
            let body = r.text().await.unwrap_or_default();
            if let Some(v) = parse_mcp_body(&body) {
                if let Some(tools) = v.pointer("/result/tools").and_then(Value::as_array) {
                    res.tool_names = tools
                        .iter()
                        .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_string))
                        .collect();
                    res.tool_count = Some(res.tool_names.len());
                    res.ok = !res.tool_names.is_empty();
                    if !res.ok {
                        res.error = Some("tools/list retornou lista vazia.".into());
                    }
                } else {
                    res.error = Some("tools/list sem 'result.tools'.".into());
                }
            } else {
                res.error = Some("Resposta do tools/list não interpretada.".into());
            }
        }
        Ok(r) => res.error = Some(format!("HTTP {} no tools/list", r.status().as_u16())),
        Err(e) => res.error = Some(sanitize(&e.to_string())),
    }
    res
}

/// Testa a ponte stdio `ai-memory.exe mcp-bridge --no-session-id` exatamente como um cliente
/// (Codex) a executaria, sem AI_MEMORY_AUTH_TOKEN no ambiente: initialize + tools/list.
pub async fn test_mcp_bridge(exe: &Path, mcp_url: &str) -> McpTest {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut res = McpTest {
        level: "ponte local (stdio)",
        endpoint: format!("{} mcp-bridge → {mcp_url}", exe.display()),
        with_token: false,
        ok: false,
        http_status: None,
        server_name: None,
        server_version: None,
        tool_count: None,
        tool_names: vec![],
        error: None,
        tested_at: chrono::Local::now().to_rfc3339(),
    };
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(["mcp-bridge", "--no-session-id", "--server-url", mcp_url])
        .env_remove("AI_MEMORY_AUTH_TOKEN")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            res.error = Some(format!("não foi possível iniciar a ponte: {e}"));
            return res;
        }
    };
    let mut stdin = child.stdin.take().expect("stdin");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let run = async {
        let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"ai-memory-manager","version":env!("CARGO_PKG_VERSION")}}});
        stdin.write_all(format!("{init}\n").as_bytes()).await.map_err(|e| e.to_string())?;
        let v = read_reply(&mut lines, 1).await?;
        if let Some(err) = v.get("error") {
            return Err(sanitize(&err.to_string()));
        }
        let name = v.pointer("/result/serverInfo/name").and_then(Value::as_str).map(str::to_string);
        let ver = v.pointer("/result/serverInfo/version").and_then(Value::as_str).map(str::to_string);
        stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await.map_err(|e| e.to_string())?;
        stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n").await.map_err(|e| e.to_string())?;
        let t = read_reply(&mut lines, 2).await?;
        if let Some(err) = t.get("error") {
            return Err(sanitize(&err.to_string()));
        }
        let tools: Vec<String> = t
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.get("name").and_then(Value::as_str).map(str::to_string)).collect())
            .unwrap_or_default();
        Ok::<_, String>((name, ver, tools))
    };
    match tokio::time::timeout(Duration::from_secs(20), run).await {
        Ok(Ok((name, ver, tools))) => {
            res.server_name = name;
            res.server_version = ver;
            res.tool_count = Some(tools.len());
            res.ok = !tools.is_empty();
            if !res.ok {
                res.error = Some("tools/list vazio pela ponte (possível falha de autenticação).".into());
            }
            res.tool_names = tools;
        }
        Ok(Err(e)) => res.error = Some(e),
        Err(_) => res.error = Some("Tempo esgotado aguardando a ponte.".into()),
    }
    let _ = child.kill().await;
    res
}

async fn read_reply<R: tokio::io::AsyncBufRead + Unpin>(lines: &mut tokio::io::Lines<R>, id: i64) -> Result<Value, String> {
    loop {
        let Some(l) = lines.next_line().await.map_err(|e| e.to_string())? else {
            return Err("a ponte encerrou sem responder".into());
        };
        if let Ok(v) = serde_json::from_str::<Value>(&l) {
            if v.get("id").and_then(Value::as_i64) == Some(id) {
                return Ok(v);
            }
        }
    }
}

// ---------------------------------------------------------------- service-host / Run

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceState {
    pub run_value_present: bool,
    pub run_value: Option<String>,
    pub run_value_points_to_binary: Option<bool>,
    pub supervisor_pid: Option<u32>,
    pub server_pid: Option<u32>,
    pub supervisor_alive: Option<bool>,
    pub server_alive: Option<bool>,
    pub stop_file_present: bool,
    pub managed_by: String,
    pub notes: Vec<String>,
}

#[cfg(windows)]
pub fn reg_get(key: &str, name: &str) -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(key)
        .ok()?
        .get_value::<String, _>(name)
        .ok()
}
#[cfg(not(windows))]
pub fn reg_get(_key: &str, _name: &str) -> Option<String> {
    None
}

#[cfg(windows)]
pub fn reg_set(key: &str, name: &str, value: Option<&str>) -> std::io::Result<()> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    let hk = RegKey::predef(HKEY_CURRENT_USER);
    match value {
        Some(v) => {
            let (k, _) = hk.create_subkey(key)?;
            k.set_value(name, &v.to_string())
        }
        None => {
            let k = hk.open_subkey_with_flags(key, KEY_SET_VALUE)?;
            match k.delete_value(name) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        }
    }
}
#[cfg(not(windows))]
pub fn reg_set(_key: &str, _name: &str, _value: Option<&str>) -> std::io::Result<()> {
    Err(std::io::Error::other("somente Windows"))
}

pub async fn pid_alive(pid: u32) -> bool {
    let spec = crate::runner::CmdSpec::new(
        r"C:\Windows\System32\tasklist.exe",
        &["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"],
    )
    .timeout(Duration::from_secs(10));
    match crate::runner::run(&spec).await {
        Ok(o) => o.stdout.contains(&format!("\"{pid}\"")),
        Err(_) => false,
    }
}

pub fn default_run_command(exe: &Path, data_dir: &Path) -> String {
    format!(
        "\"{}\" --data-dir \"{}\" --config \"{}\" service-host",
        exe.display(),
        data_dir.display(),
        data_dir.join("config.toml").display()
    )
}

pub async fn service_state(exe: Option<&Path>, data_dir: &Path) -> ServiceState {
    let mut st = ServiceState::default();
    st.run_value = reg_get(RUN_KEY, RUN_VALUE);
    st.run_value_present = st.run_value.is_some();
    if let (Some(v), Some(exe)) = (&st.run_value, exe) {
        let argv = crate::runner::split_windows_cmdline(v);
        st.run_value_points_to_binary = argv
            .first()
            .map(|a| a.eq_ignore_ascii_case(&exe.to_string_lossy()));
    }
    st.stop_file_present = data_dir.join("service-host.stop").exists();
    if let Ok(t) = std::fs::read_to_string(data_dir.join("service-host.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&t) {
            st.supervisor_pid = v.get("supervisor_pid").and_then(Value::as_u64).map(|x| x as u32);
            st.server_pid = v.get("server_pid").and_then(Value::as_u64).map(|x| x as u32);
        }
    }
    if let Some(p) = st.supervisor_pid {
        st.supervisor_alive = Some(pid_alive(p).await);
    }
    if let Some(p) = st.server_pid {
        st.server_alive = Some(pid_alive(p).await);
    }
    st.managed_by = if st.supervisor_alive == Some(true) {
        "service-host".into()
    } else if st.run_value_present {
        "service-host (inativo)".into()
    } else {
        "desconhecido".into()
    };
    if st.run_value_points_to_binary == Some(false) {
        st.notes.push("O valor Run aponta para outro executável que não o binário detectado.".into());
    }
    st
}

/// Para o servidor pelo mecanismo documentado (arquivo service-host.stop) e aguarda.
pub async fn stop_server(data_dir: &Path, base: &str) -> Result<String, String> {
    let stop = data_dir.join("service-host.stop");
    std::fs::write(&stop, b"").map_err(|e| format!("não foi possível criar {}: {e}", stop.display()))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(700)).await;
        if !health(base).await.reachable {
            return Ok("Servidor parado (healthz não responde).".into());
        }
    }
    Err("O servidor ainda responde após 20 s. Nenhum processo foi encerrado à força.".into())
}

#[cfg(windows)]
fn spawn_detached(argv: &[String]) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| ())
}
#[cfg(not(windows))]
fn spawn_detached(_argv: &[String]) -> std::io::Result<()> {
    Err(std::io::Error::other("somente Windows"))
}

/// Inicia o servidor com o mesmo comando do valor Run (ou o padrão), exigindo que o
/// executável seja exatamente o binário ai-memory detectado.
pub async fn start_server(exe: &Path, data_dir: &Path, base: &str) -> Result<String, String> {
    if health(base).await.reachable {
        return Ok("O servidor já está respondendo.".into());
    }
    let _ = std::fs::remove_file(data_dir.join("service-host.stop"));
    let cmdline = reg_get(RUN_KEY, RUN_VALUE).unwrap_or_else(|| default_run_command(exe, data_dir));
    let argv = crate::runner::split_windows_cmdline(&cmdline);
    if argv.is_empty() || !argv[0].eq_ignore_ascii_case(&exe.to_string_lossy()) {
        return Err("O comando de inicialização registrado não aponta para o binário ai-memory detectado; por segurança não foi executado.".into());
    }
    if !argv.iter().any(|a| a == "service-host") {
        return Err("O comando de inicialização registrado não é 'service-host'; não reconhecido.".into());
    }
    spawn_detached(&argv).map_err(|e| format!("falha ao iniciar: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(800)).await;
        if health(base).await.reachable {
            return Ok("Servidor iniciado (healthz respondeu).".into());
        }
    }
    Err("Processo iniciado, mas o healthz não respondeu em 25 s. Veja os logs em <data_dir>\\logs.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_and_token_from_data_dir() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("auth-tokens")).unwrap();
        std::fs::write(
            d.path().join("config.toml"),
            "bind = \"127.0.0.1:49999\"\n[auth]\nbearer_token_file = \"auth-tokens/default\"\ntoken_pepper = \"x\"\n",
        )
        .unwrap();
        std::fs::write(d.path().join("auth-tokens").join("default"), "tok-abcdef-123456\n").unwrap();
        let c = read_ai_memory_config(d.path());
        assert_eq!(server_base_url(&c), "http://127.0.0.1:49999");
        let t = read_token(d.path(), &c).unwrap();
        assert_eq!(t, "tok-abcdef-123456");
        // após lido, o token passa a ser redigido em qualquer saída
        assert!(!sanitize("valor tok-abcdef-123456").contains("tok-abcdef"));
    }

    #[test]
    fn mcp_sse_body_parsing() {
        let sse = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[{\"name\":\"a\"}]}}\n\n";
        assert!(parse_mcp_body(sse).unwrap().pointer("/result/tools").is_some());
        assert!(parse_mcp_body("{\"x\":1}").is_some());
    }

    #[tokio::test]
    async fn health_unreachable_is_not_ok() {
        let h = health("http://127.0.0.1:9").await;
        assert!(!h.reachable);
    }

    #[cfg(windows)]
    #[test]
    fn registry_roundtrip_in_test_key() {
        let key = r"Software\AIMemoryManagerTest";
        reg_set(key, "v", Some("abc")).unwrap();
        assert_eq!(reg_get(key, "v").as_deref(), Some("abc"));
        reg_set(key, "v", None).unwrap();
        assert!(reg_get(key, "v").is_none());
        reg_set(key, "v", None).unwrap(); // idempotente
        let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).delete_subkey(key);
    }
}
