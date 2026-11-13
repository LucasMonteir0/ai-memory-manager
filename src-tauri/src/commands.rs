//! Comandos Tauri estritamente tipados. O frontend nunca recebe o token bearer.

use crate::agents::{self, AgentReport, DesktopApps};
use crate::aimemory::{self, AiMemoryCli, Backfill, Doctor, Interpreted, Semver, SetupPlan, Status};
use crate::audit::{AppOperation, AuditLog};
use crate::backup::{BackupSet, BackupStore, FileDiff};
use crate::changes::{self, Action, ApplyResult, ChangePlan, Planned, Verify};
use crate::installer::{self, InstallReport, ReleaseInfo};
use crate::paths::SysPaths;
use crate::projects::{self, Catalog, FolderInspection, Project};
use crate::runner::{self, CmdOutput, CmdSpec, LineSink, StreamKind};
use crate::sanitize::sanitize;
use crate::service::{self, Health, McpTest, ServiceState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

// ---------------------------------------------------------------- estado

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub manual_binary: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureEvidence {
    pub agent: String,
    pub project_id: String,
    pub started_at: String,
    pub captured_before: u64,
    pub captured_after: Option<u64>,
    pub checked_at: Option<String>,
    pub verified: bool,
    pub message: String,
}

pub struct AppState {
    pub paths: SysPaths,
    pub settings: Mutex<Settings>,
    pub catalog: Catalog,
    pub backups: BackupStore,
    pub audit: AuditLog,
    pub plans: Mutex<HashMap<String, Planned>>,
    pub mutation: tokio::sync::Mutex<()>,
    pub cancels: Mutex<HashMap<String, watch::Sender<bool>>>,
    pub doctor_cache: Mutex<HashMap<String, (Doctor, String)>>,
    pub capture_tests: Mutex<HashMap<String, CaptureEvidence>>,
    pub apps_cache: Mutex<Option<(DesktopApps, Instant)>>,
    pub dry_runs: Mutex<HashMap<String, Instant>>,
}

impl AppState {
    pub fn new(paths: SysPaths) -> Self {
        let app_dir = paths.app_dir();
        let _ = std::fs::create_dir_all(&app_dir);
        let settings = std::fs::read(app_dir.join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            catalog: Catalog::new(&app_dir),
            backups: BackupStore::new(&app_dir),
            audit: AuditLog::new(&app_dir),
            settings: Mutex::new(settings),
            plans: Mutex::new(HashMap::new()),
            mutation: tokio::sync::Mutex::new(()),
            cancels: Mutex::new(HashMap::new()),
            doctor_cache: Mutex::new(HashMap::new()),
            capture_tests: Mutex::new(HashMap::new()),
            apps_cache: Mutex::new(None),
            dry_runs: Mutex::new(HashMap::new()),
            paths,
        }
    }

    fn save_settings(&self) {
        let s = self.settings.lock().unwrap().clone();
        let _ = crate::backup::write_atomic(
            &self.paths.app_dir().join("settings.json"),
            &serde_json::to_vec_pretty(&s).unwrap_or_default(),
        );
    }

    fn data_dir(&self) -> PathBuf {
        self.paths.ai_memory_default_data_dir()
    }

    async fn cli(&self) -> Result<(AiMemoryCli, Semver), String> {
        let manual = self.settings.lock().unwrap().manual_binary.clone();
        for (p, _src) in aimemory::candidate_paths(&self.paths, manual.as_deref().map(Path::new)) {
            if let Ok(v) = aimemory::validate_binary(&p).await {
                return Ok((AiMemoryCli { exe: p }, v));
            }
        }
        Err("ai-memory não encontrado ou inválido. Vá em Configurações para instalar ou localizar.".into())
    }

    fn base_url(&self) -> String {
        service::server_base_url(&service::read_ai_memory_config(&self.data_dir()))
    }

    fn token(&self) -> Option<String> {
        let dd = self.data_dir();
        service::read_token(&dd, &service::read_ai_memory_config(&dd))
    }

    fn lock_mutation(&self) -> Result<tokio::sync::MutexGuard<'_, ()>, String> {
        self.mutation
            .try_lock()
            .map_err(|_| "Outra operação de alteração está em andamento. Aguarde a conclusão.".to_string())
    }

    async fn apps(&self, refresh: bool) -> DesktopApps {
        if !refresh {
            if let Some((a, t)) = self.apps_cache.lock().unwrap().clone() {
                if t.elapsed() < Duration::from_secs(120) {
                    return a;
                }
            }
        }
        let a = agents::detect_desktop_apps(&self.paths).await;
        *self.apps_cache.lock().unwrap() = Some((a.clone(), Instant::now()));
        a
    }

    fn project(&self, id: &str) -> Result<Project, String> {
        self.catalog.get(id).ok_or_else(|| "Projeto não encontrado no catálogo.".into())
    }

    fn store_plan(&self, p: Planned) -> ChangePlan {
        let public = p.plan.clone();
        self.plans.lock().unwrap().insert(public.id.clone(), p);
        public
    }
}

type R<T> = Result<T, String>;

fn sink(app: &AppHandle, op_id: &str) -> LineSink {
    let app = app.clone();
    let op = op_id.to_string();
    Arc::new(move |kind: StreamKind, line: &str| {
        let _ = app.emit("op-line", serde_json::json!({"opId": op, "stream": kind, "line": line}));
    })
}

// ---------------------------------------------------------------- ambiente / detecção

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvInfo {
    pub os: String,
    pub arch: String,
    pub native_windows: bool,
    pub app_dir: String,
    pub app_version: String,
    pub install_kind: String,
    pub claude_config_dir_env: Option<String>,
    pub codex_home_env: Option<String>,
}

#[tauri::command]
pub fn env_info(state: State<'_, AppState>) -> EnvInfo {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    let installed = exe_dir.map(|d| d.join("uninstall.exe").is_file()).unwrap_or(false);
    EnvInfo {
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        native_windows: cfg!(windows),
        app_dir: state.paths.app_dir().to_string_lossy().to_string(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        install_kind: if installed { "installed".into() } else { "portable".into() },
        claude_config_dir_env: state.paths.claude_config_dir_env.as_ref().map(|p| p.to_string_lossy().to_string()),
        codex_home_env: state.paths.codex_home_env.as_ref().map(|p| p.to_string_lossy().to_string()),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    /// not_installed | invalid | unsupported_version | no_setup | server_stopped | auth_error | healthy | unknown
    pub state: String,
    pub state_label: String,
    pub binary: Option<String>,
    pub candidates: Vec<aimemory::BinaryCandidate>,
    pub version: Option<String>,
    pub version_support: Option<String>,
    pub commands: Vec<String>,
    pub has_recover: bool,
    pub has_hook_drain: bool,
    pub data_dir: String,
    pub config: service::AiMemoryConfig,
    pub server_url: String,
    pub web_url: String,
    pub health: Option<Health>,
    pub service: Option<ServiceState>,
    pub status: Option<Interpreted<Status>>,
    pub status_command: Option<CmdOutput>,
    pub token_available: bool,
    pub checked_at: String,
}

fn label(state: &str) -> &'static str {
    match state {
        "not_installed" => "Não instalado",
        "invalid" => "Binário inválido",
        "unsupported_version" => "Versão não suportada",
        "no_setup" => "Binário presente sem setup",
        "server_stopped" => "Instalado com servidor parado",
        "auth_error" => "Instalado com erro de autenticação",
        "healthy" => "Instalado e saudável",
        _ => "Não interpretado",
    }
}

#[tauri::command]
pub async fn detect(state: State<'_, AppState>) -> R<Detection> {
    let manual = state.settings.lock().unwrap().manual_binary.clone();
    let mut candidates = vec![];
    let mut chosen: Option<(PathBuf, Semver)> = None;
    for (p, src) in aimemory::candidate_paths(&state.paths, manual.as_deref().map(Path::new)) {
        if !p.is_file() {
            if src == "manual" || src == "padrao" {
                candidates.push(aimemory::BinaryCandidate { path: p.to_string_lossy().into(), source: src, valid: false, version: None, error: Some("não encontrado".into()) });
            }
            continue;
        }
        let r = aimemory::validate_binary(&p).await;
        candidates.push(aimemory::BinaryCandidate {
            path: p.to_string_lossy().into(),
            source: src,
            valid: r.is_ok(),
            version: r.as_ref().ok().map(|v| v.to_string()),
            error: r.as_ref().err().cloned(),
        });
        if chosen.is_none() {
            if let Ok(v) = r {
                chosen = Some((p, v));
            }
        }
    }
    let data_dir = state.data_dir();
    let config = service::read_ai_memory_config(&data_dir);
    let server_url = service::server_base_url(&config);
    let mut d = Detection {
        state: String::new(),
        state_label: String::new(),
        binary: None,
        candidates,
        version: None,
        version_support: None,
        commands: vec![],
        has_recover: false,
        has_hook_drain: false,
        data_dir: data_dir.to_string_lossy().into(),
        config: config.clone(),
        web_url: format!("{server_url}/web"),
        server_url: server_url.clone(),
        health: None,
        service: None,
        status: None,
        status_command: None,
        token_available: false,
        checked_at: chrono::Local::now().to_rfc3339(),
    };
    let Some((exe, ver)) = chosen else {
        d.state = if d.candidates.iter().any(|c| c.source != "padrao" || c.error.as_deref() != Some("não encontrado")) && d.candidates.iter().any(|c| c.error.as_deref() != Some("não encontrado")) {
            "invalid".into()
        } else {
            "not_installed".into()
        };
        d.state_label = label(&d.state).into();
        return Ok(d);
    };
    let cli = AiMemoryCli { exe: exe.clone() };
    d.binary = Some(exe.to_string_lossy().into());
    d.version = Some(ver.to_string());
    d.version_support = Some(aimemory::version_support(ver).into());
    if let Ok(h) = cli.help(None).await {
        let cmds = aimemory::parse_help_commands(&h.stdout);
        d.has_recover = cmds.contains("recover");
        d.has_hook_drain = cmds.contains("hook-drain");
        d.commands = cmds.into_iter().collect();
    }
    d.token_available = state.token().is_some();
    d.service = Some(service::service_state(Some(&exe), &data_dir).await);
    if aimemory::version_support(ver) == "unsupported" {
        d.state = "unsupported_version".into();
        d.state_label = label(&d.state).into();
        return Ok(d);
    }
    if !config.exists {
        d.state = "no_setup".into();
        d.state_label = label(&d.state).into();
        return Ok(d);
    }
    let h = service::health(&server_url).await;
    let reachable = h.reachable;
    d.health = Some(h);
    let out = cli.run(&["status", "--json"], None, Duration::from_secs(30)).await.map_err(|e| e.to_string())?;
    let parsed = aimemory::parse_status(&out.stdout);
    let stderr_l = out.stderr_meaningful().to_lowercase();
    d.state = match (&parsed, reachable) {
        (_, false) => "server_stopped".into(),
        (Interpreted::Ok { data }, true) => {
            if data.auth_fix.is_some() || stderr_l.contains("401") || stderr_l.contains("unauthorized") {
                "auth_error".into()
            } else {
                "healthy".into()
            }
        }
        (Interpreted::Raw { .. }, true) => {
            if stderr_l.contains("401") || stderr_l.contains("unauthorized") { "auth_error".into() } else { "unknown".into() }
        }
    };
    d.state_label = label(&d.state).into();
    d.status = Some(parsed);
    d.status_command = Some(out);
    Ok(d)
}

#[tauri::command]
pub async fn set_manual_binary(state: State<'_, AppState>, path: Option<String>) -> R<String> {
    if let Some(p) = &path {
        let v = aimemory::validate_binary(Path::new(p)).await.map_err(|e| format!("Executável rejeitado: {e}"))?;
        state.settings.lock().unwrap().manual_binary = Some(p.clone());
        state.save_settings();
        return Ok(format!("ai-memory {} validado.", v.to_string()));
    }
    state.settings.lock().unwrap().manual_binary = None;
    state.save_settings();
    Ok("Seleção manual removida.".into())
}

// ---------------------------------------------------------------- projetos

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Vec<Project> {
    state.catalog.load()
}

#[tauri::command]
pub fn inspect_folder(state: State<'_, AppState>, path: String) -> R<FolderInspection> {
    projects::inspect_folder(&path, &state.paths.home)
}

#[tauri::command]
pub fn add_project(state: State<'_, AppState>, path: String, display_name: Option<String>) -> R<Project> {
    let dir = projects::normalize_folder(&path)?;
    let p = state.catalog.add(&dir, display_name)?;
    let op = state.audit.start("Adicionar projeto ao catálogo", "alteracao", Some(&p.id), None);
    state.audit.finish(op, true, &format!("{} adicionado (somente catálogo da GUI)", p.absolute_path), "");
    Ok(p)
}

#[tauri::command]
pub fn remove_project(state: State<'_, AppState>, id: String) -> R<String> {
    let p = state.project(&id)?;
    state.catalog.remove(&id)?;
    let op = state.audit.start("Remover projeto do catálogo", "alteracao", Some(&id), None);
    state.audit.finish(op, true, &format!("{} removido apenas do catálogo; memórias, marker e configs intactos", p.absolute_path), "");
    Ok("Removido apenas da lista da GUI. Nenhuma memória foi apagada.".into())
}

#[tauri::command]
pub fn rename_project(state: State<'_, AppState>, id: String, display_name: String) -> R<Project> {
    if display_name.trim().is_empty() {
        return Err("Nome vazio.".into());
    }
    state.catalog.update(&id, |p| p.display_name = display_name.trim().to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorRun {
    pub project_id: String,
    pub cwd: String,
    pub ran_at: String,
    pub test_kind: String,
    pub output: CmdOutput,
    pub parsed: Interpreted<Doctor>,
}

#[tauri::command]
pub async fn run_doctor(state: State<'_, AppState>, project_id: String) -> R<DoctorRun> {
    let p = state.project(&project_id)?;
    let (cli, _) = state.cli().await?;
    let dir = projects::normalize_folder(&p.absolute_path)?;
    let op = state.audit.start("doctor", "leitura", Some(&project_id), None);
    let out = cli
        .run(&["doctor", "--json", "--language", "en"], Some(&dir), Duration::from_secs(90))
        .await
        .map_err(|e| e.to_string())?;
    let parsed = aimemory::parse_doctor(&out.stdout);
    let now = chrono::Local::now().to_rfc3339();
    let ok = matches!(parsed, Interpreted::Ok { .. });
    if let Interpreted::Ok { data } = &parsed {
        state.doctor_cache.lock().unwrap().insert(project_id.clone(), (data.clone(), now.clone()));
        let (ws, pr) = (data.workspace.clone(), data.project.clone());
        let _ = state.catalog.update(&project_id, |pp| {
            pp.last_doctor_at = Some(now.clone());
            pp.last_workspace = Some(ws);
            pp.last_project = Some(pr);
        });
    }
    state.audit.finish(
        op,
        ok && out.ok(),
        &format!("doctor em {} — {}", dir.display(), if ok { "interpretado" } else { "NÃO interpretado" }),
        &format!("$ {}\n(cwd {})\n\n{}\n{}", out.command, dir.display(), out.stdout, out.stderr),
    );
    Ok(DoctorRun { project_id, cwd: dir.to_string_lossy().into(), ran_at: now, test_kind: "configuração (doctor)".into(), output: out, parsed })
}

#[tauri::command]
pub fn cached_doctor(state: State<'_, AppState>, project_id: String) -> Option<(Doctor, String)> {
    state.doctor_cache.lock().unwrap().get(&project_id).cloned()
}

#[tauri::command]
pub fn plan_marker(state: State<'_, AppState>, project_id: String, workspace: String, project: String) -> R<ChangePlan> {
    let p = state.project(&project_id)?;
    let dir = projects::normalize_folder(&p.absolute_path)?;
    let target = dir.join(".ai-memory.toml");
    if target.exists() {
        return Err("Já existe um .ai-memory.toml nesta pasta; ele não será sobrescrito.".into());
    }
    let content = projects::marker_content(workspace.trim(), project.trim())?;
    let mut plan = changes::new_plan(
        "Criar marker .ai-memory.toml",
        "Fixa a identidade do projeto (workspace/projeto) para todas as sessões abertas nesta pasta ou abaixo dela. Usa apenas as chaves documentadas 'workspace' e 'project'.",
    );
    plan.project_id = Some(project_id);
    plan.files = vec![target.to_string_lossy().into()];
    plan.diffs = vec![FileDiff::new(&target.to_string_lossy(), "", &content, false)];
    if dir.join(".git").exists() {
        plan.warnings.push("O arquivo aparecerá no `git status`; considere versioná-lo para que todos os checkouts usem a mesma identidade.".into());
    }
    plan.warnings.push("Se o nome diferir do atual, novas sessões serão gravadas sob a nova identidade (o histórico existente não é movido).".into());
    plan.after_steps = vec!["Rode o doctor novamente para confirmar a identidade (scope_source = marker).".into()];
    Ok(state.store_plan(Planned {
        plan,
        action: Action::CreateMarker { dir, content },
        backup_files: vec![],
        verify: Verify::FileExists { file: target },
    }))
}

// ---------------------------------------------------------------- backfill / recover

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackfillRun {
    pub output: CmdOutput,
    pub parsed: Interpreted<Backfill>,
    pub dry_run: bool,
    pub ran_at: String,
}

fn backfill_args(ws: &str, pr: &str, force: bool, max: u32, dry: bool) -> Vec<String> {
    let mut a = vec!["backfill".to_string(), "--json".into(), "--workspace".into(), ws.into(), "--project".into(), pr.into(), "--max-sessions".into(), max.max(1).min(10000).to_string()];
    if force {
        a.push("--force".into());
    }
    if dry {
        a.push("--dry-run".into());
    }
    a
}

fn dry_key(project_id: &str, force: bool, max: u32) -> String {
    format!("{project_id}|{force}|{max}")
}

async fn project_identity(state: &AppState, cli: &AiMemoryCli, p: &Project) -> R<(String, String, PathBuf)> {
    let dir = projects::normalize_folder(&p.absolute_path)?;
    if let Some((d, _)) = state.doctor_cache.lock().unwrap().get(&p.id).cloned() {
        return Ok((d.workspace, d.project, dir));
    }
    let out = cli.run(&["doctor", "--json", "--language", "en"], Some(&dir), Duration::from_secs(90)).await.map_err(|e| e.to_string())?;
    match aimemory::parse_doctor(&out.stdout) {
        Interpreted::Ok { data } => Ok((data.workspace, data.project, dir)),
        Interpreted::Raw { reason, .. } => Err(format!("Não foi possível resolver a identidade pelo doctor: {reason}")),
    }
}

#[tauri::command]
pub async fn backfill_dry_run(state: State<'_, AppState>, project_id: String, force: bool, max_sessions: u32) -> R<BackfillRun> {
    let p = state.project(&project_id)?;
    let (cli, _) = state.cli().await?;
    let (ws, pr, dir) = project_identity(&state, &cli, &p).await?;
    let spec = CmdSpec::new(&cli.exe, &[]).args_owned(backfill_args(&ws, &pr, force, max_sessions, true)).cwd(&dir).timeout(Duration::from_secs(300));
    let op = state.audit.start("backfill --dry-run", "leitura", Some(&project_id), None);
    let out = runner::run(&spec).await.map_err(|e| e.to_string())?;
    let parsed = aimemory::parse_backfill(&out.stdout);
    if out.ok() && matches!(parsed, Interpreted::Ok { .. }) {
        state.dry_runs.lock().unwrap().insert(dry_key(&project_id, force, max_sessions), Instant::now());
    }
    state.audit.finish(op, out.ok(), &format!("simulação de backfill {ws}/{pr}"), &format!("$ {}\n{}\n{}", out.command, out.stdout, out.stderr));
    Ok(BackfillRun { output: out, parsed, dry_run: true, ran_at: chrono::Local::now().to_rfc3339() })
}

#[tauri::command]
pub async fn backfill_run(app: AppHandle, state: State<'_, AppState>, project_id: String, force: bool, max_sessions: u32, op_id: String) -> R<BackfillRun> {
    let key = dry_key(&project_id, force, max_sessions);
    let fresh = state.dry_runs.lock().unwrap().get(&key).map(|t| t.elapsed() < Duration::from_secs(30 * 60)).unwrap_or(false);
    if !fresh {
        return Err("Execute a simulação (dry-run) com os mesmos parâmetros antes de importar.".into());
    }
    let _g = state.lock_mutation()?;
    let p = state.project(&project_id)?;
    let (cli, _) = state.cli().await?;
    let (ws, pr, dir) = project_identity(&state, &cli, &p).await?;
    let spec = CmdSpec::new(&cli.exe, &[]).args_owned(backfill_args(&ws, &pr, force, max_sessions, false)).cwd(&dir).timeout(Duration::from_secs(60 * 60));
    let (tx, rx) = watch::channel(false);
    state.cancels.lock().unwrap().insert(op_id.clone(), tx);
    let op = state.audit.start(if force { "backfill --force" } else { "backfill" }, "alteracao", Some(&project_id), None);
    let res = runner::run_streaming(&spec, Some(sink(&app, &op_id)), Some(rx)).await;
    state.cancels.lock().unwrap().remove(&op_id);
    state.dry_runs.lock().unwrap().remove(&key);
    let out = res.map_err(|e| e.to_string())?;
    let parsed = aimemory::parse_backfill(&out.stdout);
    state.audit.finish(
        op,
        out.ok(),
        &format!("backfill {ws}/{pr}{}", if out.cancelled { " (cancelado)" } else { "" }),
        &format!("$ {}\n{}\n{}", out.command, out.stdout, out.stderr),
    );
    state.doctor_cache.lock().unwrap().remove(&project_id);
    Ok(BackfillRun { output: out, parsed, dry_run: false, ran_at: chrono::Local::now().to_rfc3339() })
}

#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>, op_id: String) -> bool {
    if let Some(tx) = state.cancels.lock().unwrap().get(&op_id) {
        let _ = tx.send(true);
        return true;
    }
    false
}

#[tauri::command]
pub async fn recover_run(app: AppHandle, state: State<'_, AppState>, op_id: String) -> R<CmdOutput> {
    let _g = state.lock_mutation()?;
    let (cli, _) = state.cli().await?;
    let help = cli.help(None).await.map_err(|e| e.to_string())?;
    let cmds = aimemory::parse_help_commands(&help.stdout);
    let args: Vec<String> = if cmds.contains("recover") {
        vec!["recover".into(), "--json".into()]
    } else if cmds.contains("hook-drain") {
        vec!["hook-drain".into()]
    } else {
        return Err("Esta versão do ai-memory não oferece 'recover' nem 'hook-drain'.".into());
    };
    let spec = CmdSpec::new(&cli.exe, &[]).args_owned(args).timeout(Duration::from_secs(30 * 60));
    let (tx, rx) = watch::channel(false);
    state.cancels.lock().unwrap().insert(op_id.clone(), tx);
    let op = state.audit.start("drenar spool (recover)", "alteracao", None, None);
    let out = runner::run_streaming(&spec, Some(sink(&app, &op_id)), Some(rx)).await;
    state.cancels.lock().unwrap().remove(&op_id);
    let out = out.map_err(|e| e.to_string())?;
    state.audit.finish(op, out.ok(), "recover", &format!("$ {}\n{}\n{}", out.command, out.stdout, out.stderr));
    Ok(out)
}

// ---------------------------------------------------------------- agentes

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsView {
    pub apps: DesktopApps,
    pub agents: Vec<AgentReport>,
    pub doctor_source: Option<String>,
    pub capture_tests: Vec<CaptureEvidence>,
}

fn any_doctor(state: &AppState, project_id: Option<&str>) -> Option<(Doctor, String)> {
    let cache = state.doctor_cache.lock().unwrap();
    if let Some(id) = project_id {
        if let Some(d) = cache.get(id) {
            return Some(d.clone());
        }
    }
    cache.values().max_by(|a, b| a.1.cmp(&b.1)).cloned()
}

#[tauri::command]
pub async fn agents_view(state: State<'_, AppState>, project_id: Option<String>, refresh: bool) -> R<AgentsView> {
    let apps = state.apps(refresh).await;
    let doctor = any_doctor(&state, project_id.as_deref());
    let requires_token = state.token().is_some();
    let mut reports = {
        let ctx = agents::Ctx { paths: &state.paths, apps: &apps, doctor: doctor.as_ref().map(|d| &d.0), server_requires_token: requires_token };
        agents::all_reports(&ctx)
    };
    let tests: Vec<CaptureEvidence> = state.capture_tests.lock().unwrap().values().cloned().collect();
    for r in reports.iter_mut() {
        if let Some(t) = tests.iter().find(|t| t.agent == r.id && t.verified) {
            r.hooks.status = agents::IntegrationStatus::Verified;
            r.hooks.evidence.insert(0, format!("Captura confirmada com sessão nova em {} ({} → {}).", t.checked_at.clone().unwrap_or_default(), t.captured_before, t.captured_after.unwrap_or(0)));
        }
    }
    Ok(AgentsView {
        apps,
        agents: reports,
        doctor_source: doctor.map(|(d, at)| format!("doctor {}/{} em {at}", d.workspace, d.project)),
        capture_tests: tests,
    })
}

#[tauri::command]
pub async fn plan_agent_change(state: State<'_, AppState>, agent: String, component: String, action: String) -> R<ChangePlan> {
    if !["mcp", "hooks"].contains(&component.as_str()) || !["enable", "disable"].contains(&action.as_str()) {
        return Err("Parâmetros inválidos.".into());
    }
    let apps = state.apps(false).await;
    let cd_cfg = if agent == "claude-desktop" { Some(agents::resolve_claude_desktop_config(&state.paths, &apps)?) } else { None };
    let doctor = any_doctor(&state, None);
    let ctx = agents::Ctx { paths: &state.paths, apps: &apps, doctor: doctor.as_ref().map(|d| &d.0), server_requires_token: true };
    let report = agents::all_reports(&ctx).into_iter().find(|r| r.id == agent).ok_or("Agente desconhecido.")?;
    let comp = if component == "mcp" { &report.mcp } else { &report.hooks };
    let file = agents::primary_config_file(&state.paths, &agent, &component, cd_cfg.as_deref()).ok_or("Integração não suportada para este agente.")?;
    let planned = if action == "disable" {
        if !comp.can_disable {
            return Err("Não há entradas do ai-memory para desativar.".into());
        }
        if component == "mcp" { changes::plan_disable_mcp(&agent, &file)? } else { changes::plan_disable_hooks(&agent, &file)? }
    } else {
        if !comp.can_configure {
            return Err(comp.blocked_reason.clone().unwrap_or_else(|| "Configuração bloqueada.".into()));
        }
        let (cli, _) = state.cli().await?;
        let client_id = if component == "mcp" { report.mcp_client_id.clone() } else { report.hooks_agent_id.clone() }.ok_or("Sem suporte.")?;
        let extra: Vec<PathBuf> = agents::agent_files(&state.paths, &agent, &component, cd_cfg.as_deref()).into_iter().skip(1).collect();
        let mut p = changes::plan_official_install(&cli.exe, &agent, &component, &client_id, &file, extra).await?;
        let cursor_compat = agent == "claude-code" && component == "hooks" && apps.cursor.installed == Some(true);
        if let (true, Action::Official { flatten_hooks, .. }) = (cursor_compat, &mut p.action) {
            *flatten_hooks = Some(file.clone());
            p.plan.warnings.insert(
                0,
                "Cursor detectado: depois do instalador, os hooks do ai-memory são reescritos em linha única (command sem args). O Cursor importa os hooks do Claude Code ignorando `args` e bloquearia os prompts.".into(),
            );
        }
        if matches!(comp.status, agents::IntegrationStatus::Configured | agents::IntegrationStatus::Partial | agents::IntegrationStatus::Untrusted | agents::IntegrationStatus::Verified) {
            p.plan.warnings.insert(0, "Já existe configuração: o instalador oficial substitui apenas a entrada ai-memory (idempotente), sem duplicar.".into());
        }
        p.plan.after_steps.insert(0, report.restart_hint.clone());
        p
    };
    Ok(state.store_plan(planned))
}

/// Q16: corrige o token do MCP do Codex criando a variável de usuário (segredo no registro).
#[tauri::command]
pub fn plan_codex_token_fix(state: State<'_, AppState>) -> R<ChangePlan> {
    let cfg = std::fs::read_to_string(state.paths.codex_home().join("config.toml")).map_err(|_| "config.toml do Codex não encontrado.")?;
    let m = crate::configedit::codex_mcp(&cfg, crate::configedit::DEFAULT_MCP_NAME).map_err(|e| e.to_string())?;
    let var = m.bearer_token_env_var.ok_or("A entrada do Codex não usa bearer_token_env_var.")?;
    if !var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Nome de variável inesperado.".into());
    }
    if service::reg_get("Environment", &var).is_some() {
        return Err(format!("A variável {var} já existe no ambiente do usuário; nada a fazer."));
    }
    let token = state.token().ok_or("Token do ai-memory não encontrado no diretório de dados.")?;
    let mut plan = changes::new_plan(
        "Corrigir autenticação do MCP do Codex",
        &format!("Cria a variável de ambiente de usuário {var} com o token bearer do ai-memory, que o Codex lê para autenticar no MCP."),
    );
    plan.agent_id = Some("codex".into());
    plan.destructive = false;
    plan.files = vec![format!("HKCU\\Environment\\{var}")];
    plan.diffs = vec![FileDiff::new(&format!("HKCU\\Environment\\{var}"), "", &format!("{var} = {}\n", crate::sanitize::REDACTED), false)];
    plan.warnings = vec![
        "O token ficará gravado no registro do usuário (texto legível por programas que rodam com o seu usuário).".into(),
        "Todo processo iniciado depois disso herdará a variável.".into(),
        "Alternativa sem gravar segredo: manter o MCP do Codex desconectado e usar só hooks.".into(),
    ];
    plan.after_steps = vec!["Feche e reabra o Codex Desktop (ele só lê variáveis novas ao iniciar).".into(), "Use 'Testar MCP' e uma sessão nova no Codex para confirmar.".into()];
    Ok(state.store_plan(Planned {
        plan,
        action: Action::Registry { key: "Environment".into(), name: var.clone(), new_value: Some(token), expected_old_present: false, secret: true },
        backup_files: vec![],
        verify: Verify::RegistryValue { key: "Environment".into(), name: var, present: true },
    }))
}

/// Q16 (recomendado): migra o MCP do Codex para a ponte local, testando a ponte antes.
#[tauri::command]
pub async fn plan_codex_bridge(state: State<'_, AppState>) -> R<ChangePlan> {
    let (cli, _) = state.cli().await?;
    let mcp_url = format!("{}/mcp", state.base_url());
    let probe = service::test_mcp_bridge(&cli.exe, &mcp_url).await;
    let op = state.audit.start("teste da ponte MCP local (stdio)", "teste", None, Some("codex"));
    state.audit.finish(op, probe.ok, &format!("ponte: ok={} ferramentas={:?} erro={:?}", probe.ok, probe.tool_count, probe.error), "");
    if !probe.ok {
        return Err(format!("A ponte local não respondeu corretamente; nada será alterado. {}", probe.error.unwrap_or_default()));
    }
    let mut p = changes::plan_codex_bridge(&state.paths.codex_home().join("config.toml"), &cli.exe, &mcp_url)?;
    p.plan.warnings[1] = format!(
        "Ponte testada agora: {} {} respondeu com {} ferramentas, sem a variável de token.",
        probe.server_name.unwrap_or_default(),
        probe.server_version.unwrap_or_default(),
        probe.tool_count.unwrap_or(0)
    );
    Ok(state.store_plan(p))
}

/// Converte os hooks ai-memory do Claude Code para linha única (compatível com o Cursor).
#[tauri::command]
pub fn plan_cursor_compat_hooks(state: State<'_, AppState>) -> R<ChangePlan> {
    let p = changes::plan_cursor_compat_hooks(&state.paths.claude_settings())?;
    Ok(state.store_plan(p))
}

#[tauri::command]
pub fn get_plan(state: State<'_, AppState>, plan_id: String) -> Option<ChangePlan> {
    state.plans.lock().unwrap().get(&plan_id).map(|p| p.plan.clone())
}

#[tauri::command]
pub fn discard_plan(state: State<'_, AppState>, plan_id: String) {
    state.plans.lock().unwrap().remove(&plan_id);
}

#[tauri::command]
pub async fn apply_plan(state: State<'_, AppState>, plan_id: String) -> R<ApplyResult> {
    let _g = state.lock_mutation()?;
    let planned = state.plans.lock().unwrap().remove(&plan_id).ok_or("Prévia expirada ou já aplicada. Gere a prévia novamente.")?;
    let op = state.audit.start(&planned.plan.title, "alteracao", planned.plan.project_id.as_deref(), planned.plan.agent_id.as_deref());
    let res = changes::apply(&planned, &state.backups).await;
    let mut op = op;
    op.backup_id = res.backup_id.clone();
    let detail = format!(
        "{}\n\nArquivos: {}\n\nVerificação: {}\n\nSaída:\n{}\n\nDiffs:\n{}",
        res.summary,
        planned.plan.files.join(", "),
        res.verification,
        res.output.as_ref().map(|o| format!("$ {}\n{}\n{}", o.command, o.stdout, o.stderr)).unwrap_or_default(),
        res.diffs.iter().map(|d| d.unified.clone()).collect::<Vec<_>>().join("\n")
    );
    state.audit.finish(op, res.success, &res.summary, &detail);
    // força nova detecção de estado
    *state.apps_cache.lock().unwrap() = None;
    Ok(res)
}

// ---------------------------------------------------------------- testes MCP e captura

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTests {
    pub with_token: McpTest,
    pub without_token: McpTest,
    pub interpretation: String,
}

#[tauri::command]
pub async fn test_mcp(state: State<'_, AppState>) -> R<McpTests> {
    let base = state.base_url();
    let token = state.token();
    let with = service::test_mcp(&base, token.as_deref()).await;
    let without = service::test_mcp(&base, None).await;
    let interpretation = match (with.ok, without.ok, without.http_status) {
        (true, false, Some(401 | 403)) => "Servidor MCP responde e exige token: clientes sem token (ex.: variável ausente no Codex) falharão.".into(),
        (true, true, _) => "Servidor MCP responde com e sem token (autenticação não exigida para MCP).".into(),
        (true, false, _) => "Servidor MCP responde com token.".into(),
        (false, _, _) => format!("Falha no teste: {}", with.error.clone().unwrap_or_default()),
    };
    let op = state.audit.start("teste MCP (servidor)", "teste", None, None);
    state.audit.finish(op, with.ok, &interpretation, &format!("{with:#?}\n{without:#?}"));
    Ok(McpTests { with_token: with, without_token: without, interpretation })
}

fn captured_for(d: &Doctor, agent: &str) -> u64 {
    d.rows.iter().find(|r| r.agent == agent).map(|r| r.captured).unwrap_or(0)
}

async fn fresh_doctor(state: &AppState, project_id: &str) -> R<Doctor> {
    let p = state.project(project_id)?;
    let (cli, _) = state.cli().await?;
    let dir = projects::normalize_folder(&p.absolute_path)?;
    let out = cli.run(&["doctor", "--json", "--language", "en"], Some(&dir), Duration::from_secs(90)).await.map_err(|e| e.to_string())?;
    match aimemory::parse_doctor(&out.stdout) {
        Interpreted::Ok { data } => {
            state.doctor_cache.lock().unwrap().insert(project_id.to_string(), (data.clone(), chrono::Local::now().to_rfc3339()));
            Ok(data)
        }
        Interpreted::Raw { reason, .. } => Err(format!("doctor não interpretado: {reason}")),
    }
}

#[tauri::command]
pub async fn capture_test_start(state: State<'_, AppState>, agent: String, project_id: String) -> R<CaptureEvidence> {
    let d = fresh_doctor(&state, &project_id).await?;
    let ev = CaptureEvidence {
        agent: agent.clone(),
        project_id,
        started_at: chrono::Local::now().to_rfc3339(),
        captured_before: captured_for(&d, &agent),
        captured_after: None,
        checked_at: None,
        verified: false,
        message: "Abra uma NOVA sessão no agente, dentro da pasta do projeto, envie um prompt simples e encerre. Depois clique em 'Conferir'.".into(),
    };
    state.capture_tests.lock().unwrap().insert(agent, ev.clone());
    Ok(ev)
}

#[tauri::command]
pub async fn capture_test_check(state: State<'_, AppState>, agent: String) -> R<CaptureEvidence> {
    let mut ev = state.capture_tests.lock().unwrap().get(&agent).cloned().ok_or("Inicie o teste de captura primeiro.")?;
    let d = fresh_doctor(&state, &ev.project_id).await?;
    let after = captured_for(&d, &agent);
    ev.captured_after = Some(after);
    ev.checked_at = Some(chrono::Local::now().to_rfc3339());
    ev.verified = after > ev.captured_before;
    ev.message = if ev.verified {
        format!("Captura fim-a-fim confirmada: sessões capturadas {} → {}.", ev.captured_before, after)
    } else {
        "Nenhuma sessão nova capturada ainda. Confirme que a sessão foi aberta na pasta do projeto e encerrada; o spool pode levar alguns segundos.".into()
    };
    let op = state.audit.start("teste de captura (fim-a-fim)", "teste", Some(&ev.project_id), Some(&agent));
    state.audit.finish(op, ev.verified, &ev.message, "");
    state.capture_tests.lock().unwrap().insert(agent, ev.clone());
    Ok(ev)
}

// ---------------------------------------------------------------- servidor

#[tauri::command]
pub async fn server_stop(state: State<'_, AppState>) -> R<String> {
    let _g = state.lock_mutation()?;
    let op = state.audit.start("parar servidor", "alteracao", None, None);
    let r = service::stop_server(&state.data_dir(), &state.base_url()).await;
    state.audit.finish(op, r.is_ok(), r.as_ref().unwrap_or_else(|e| e), "");
    r
}

#[tauri::command]
pub async fn server_start(state: State<'_, AppState>) -> R<String> {
    let _g = state.lock_mutation()?;
    let (cli, _) = state.cli().await?;
    let op = state.audit.start("iniciar servidor", "alteracao", None, None);
    let r = service::start_server(&cli.exe, &state.data_dir(), &state.base_url()).await;
    state.audit.finish(op, r.is_ok(), r.as_ref().unwrap_or_else(|e| e), "");
    r
}

#[tauri::command]
pub async fn plan_autostart(state: State<'_, AppState>, enable: bool) -> R<ChangePlan> {
    let current = service::reg_get(service::RUN_KEY, service::RUN_VALUE);
    let (title, new_value, explanation) = if enable {
        if current.is_some() {
            return Err("O início com o Windows já está ativo.".into());
        }
        // restaura o último valor salvo, senão o comando padrão do ai-memory
        let last = state
            .backups
            .list()
            .into_iter()
            .flat_map(|b| b.registry)
            .find(|r| r.name == service::RUN_VALUE && r.value.is_some())
            .and_then(|r| r.value);
        let v = match last {
            Some(v) => v,
            None => {
                let (cli, _) = state.cli().await?;
                service::default_run_command(&cli.exe, &state.data_dir())
            }
        };
        ("Ativar início com o Windows", Some(v), "Recria o valor 'ai-memory' em HKCU\\...\\Run para iniciar o service-host ao entrar no Windows (sem admin).")
    } else {
        if current.is_none() {
            return Err("O início com o Windows já está desativado.".into());
        }
        ("Desativar início com o Windows", None, "Remove o valor 'ai-memory' de HKCU\\...\\Run. O servidor atual continua rodando até ser parado ou até o próximo logon. O valor original fica no backup para restauração.")
    };
    let mut plan = changes::new_plan(title, explanation);
    let key = format!("HKCU\\{}\\{}", service::RUN_KEY, service::RUN_VALUE);
    plan.files = vec![key.clone()];
    plan.diffs = vec![FileDiff::new(&key, &current.clone().map(|c| c + "\n").unwrap_or_default(), &new_value.clone().map(|c| c + "\n").unwrap_or_default(), current.is_some())];
    if !enable {
        plan.warnings.push("Hooks e MCP continuam configurados; com o servidor parado, eventos ficam no spool local.".into());
    }
    Ok(state.store_plan(Planned {
        plan,
        action: Action::Registry { key: service::RUN_KEY.into(), name: service::RUN_VALUE.into(), new_value, expected_old_present: current.is_some(), secret: false },
        backup_files: vec![],
        verify: Verify::RegistryValue { key: service::RUN_KEY.into(), name: service::RUN_VALUE.into(), present: enable },
    }))
}

// ---------------------------------------------------------------- backups / logs / relatório

#[tauri::command]
pub fn list_backups(state: State<'_, AppState>) -> Vec<BackupSet> {
    state.backups.list()
}

#[tauri::command]
pub fn plan_restore_backup(state: State<'_, AppState>, id: String) -> R<ChangePlan> {
    let set = state.backups.get(&id).ok_or("Backup não encontrado.")?;
    let mut plan = changes::new_plan(
        &format!("Restaurar backup {}", set.id),
        &format!("Volta os arquivos ao estado de {} ({}).", set.created_at, set.reason),
    );
    plan.agent_id = set.agent_id.clone();
    plan.files = set.files.iter().map(|f| f.original_path.clone()).chain(set.registry.iter().map(|r| format!("HKCU\\{}\\{}", r.key, r.name))).collect();
    // diff: estado atual -> backup
    plan.diffs = set
        .files
        .iter()
        .map(|f| {
            let cur = std::fs::read_to_string(&f.original_path).unwrap_or_default();
            let bak = state.backups.backup_content(&set, f).unwrap_or_default();
            FileDiff::new(&f.original_path, &cur, &bak, Path::new(&f.original_path).exists())
        })
        .collect();
    plan.warnings.push("Antes de restaurar, um novo backup do estado atual é criado.".into());
    let files: Vec<PathBuf> = set.files.iter().map(|f| PathBuf::from(&f.original_path)).collect();
    Ok(state.store_plan(Planned { plan, action: Action::RestoreBackup { id }, backup_files: files, verify: Verify::None }))
}

#[tauri::command]
pub fn list_operations(state: State<'_, AppState>, limit: Option<usize>) -> Vec<AppOperation> {
    state.audit.list(limit.unwrap_or(500))
}

#[tauri::command]
pub fn operation_detail(state: State<'_, AppState>, id: String) -> Option<String> {
    state.audit.detail(&id)
}

#[tauri::command]
pub async fn sanitized_report(state: State<'_, AppState>) -> R<String> {
    let det = detect(state.clone()).await?;
    let view = agents_view(state.clone(), None, false).await.ok();
    let mut s = String::new();
    s.push_str(&format!("# Relatório AI Memory Manager {}\nGerado em {}\n\n", env!("CARGO_PKG_VERSION"), chrono::Local::now().to_rfc3339()));
    s.push_str(&format!("Estado: {} ({})\nBinário: {:?}\nVersão: {:?} [{:?}]\nServidor: {}\nDados: {}\nToken disponível: {}\n\n", det.state_label, det.state, det.binary, det.version, det.version_support, det.server_url, det.data_dir, det.token_available));
    if let Some(Interpreted::Ok { data }) = &det.status {
        s.push_str(&format!("Status: páginas={:?} sessões={:?} observações={:?} spool={:?} embeddings={:?} captura={:?}\n\n", data.pages, data.sessions, data.observations, data.spool_pending, data.embedding_status, data.capture_mode));
    }
    if let Some(sv) = &det.service {
        s.push_str(&format!("Serviço: {} run={} supervisor_vivo={:?} servidor_vivo={:?}\n\n", sv.managed_by, sv.run_value_present, sv.supervisor_alive, sv.server_alive));
    }
    if let Some(v) = view {
        for a in v.agents {
            s.push_str(&format!("## {}\n- app: {:?} {:?}\n- MCP: {:?} — {}\n- Hooks: {:?} — {}\n", a.display_name, a.app.installed, a.app.version, a.mcp.status, a.mcp.detail, a.hooks.status, a.hooks.detail));
            for e in a.mcp.evidence.iter().chain(a.hooks.evidence.iter()) {
                s.push_str(&format!("  - {e}\n"));
            }
        }
    }
    s.push_str("\n## Projetos\n");
    for p in state.catalog.load() {
        s.push_str(&format!("- {} → {:?}/{:?} (doctor {:?})\n", p.display_name, p.last_workspace, p.last_project, p.last_doctor_at));
    }
    s.push_str("\n## Últimas operações\n");
    for o in state.audit.list(30) {
        s.push_str(&format!("- {} {} [{}] sucesso={:?}: {}\n", o.started_at, o.action, o.kind, o.success, o.summary.unwrap_or_default()));
    }
    Ok(sanitize(&s))
}

// ---------------------------------------------------------------- instalação / setup / upgrade / uninstall

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseView {
    pub release: ReleaseInfo,
    pub installed_version: Option<String>,
    pub update_available: bool,
    pub target_dir: String,
}

#[tauri::command]
pub async fn ai_memory_release(state: State<'_, AppState>) -> R<ReleaseView> {
    let rel = installer::latest_release().await?;
    let installed = state.cli().await.ok().map(|(_, v)| v);
    let latest = rel.version.as_deref().and_then(Semver::parse);
    Ok(ReleaseView {
        update_available: matches!((installed, latest), (Some(i), Some(l)) if l > i),
        installed_version: installed.map(|v| v.to_string()),
        target_dir: state.paths.ai_memory_default_install_dir().to_string_lossy().into(),
        release: rel,
    })
}

#[tauri::command]
pub async fn install_ai_memory(state: State<'_, AppState>, add_to_path: bool) -> R<InstallReport> {
    let _g = state.lock_mutation()?;
    if state.cli().await.is_ok() {
        return Err("O ai-memory já está instalado; a GUI não reinstala uma instalação existente.".into());
    }
    let rel = installer::latest_release().await?;
    let op = state.audit.start("instalar ai-memory", "alteracao", None, None);
    let r = installer::install(&rel, &state.paths.ai_memory_default_install_dir(), add_to_path).await;
    state.audit.finish(op, r.is_ok(), &r.as_ref().map(|x| format!("instalado {}", x.version)).unwrap_or_else(|e| e.clone()), &format!("{r:#?}"));
    r
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlanView {
    pub parsed: Interpreted<SetupPlan>,
    pub output: CmdOutput,
    pub already_configured: bool,
}

#[tauri::command]
pub async fn setup_plan(state: State<'_, AppState>) -> R<SetupPlanView> {
    let (cli, _) = state.cli().await?;
    let out = cli.run(&["setup", "--plan", "--json", "--language", "en"], None, Duration::from_secs(60)).await.map_err(|e| e.to_string())?;
    let parsed = aimemory::parse_setup_plan(&out.stdout);
    Ok(SetupPlanView { parsed, output: out, already_configured: state.data_dir().join("config.toml").exists() })
}

#[tauri::command]
pub async fn setup_apply(app: AppHandle, state: State<'_, AppState>, agents_sel: Vec<String>, op_id: String) -> R<CmdOutput> {
    if state.data_dir().join("config.toml").exists() {
        return Err("Já existe configuração do ai-memory; o setup não será executado novamente.".into());
    }
    let _g = state.lock_mutation()?;
    let (cli, _) = state.cli().await?;
    let mut args = vec!["setup".to_string(), "--yes".into(), "--language".into(), "en".into()];
    for a in &agents_sel {
        if !a.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(format!("Agente inválido: {a}"));
        }
        args.push("--agent".into());
        args.push(a.clone());
    }
    let mut files = vec![];
    for a in &agents_sel {
        for c in ["mcp", "hooks"] {
            files.extend(agents::agent_files(&state.paths, a, c, None));
        }
    }
    let set = state.backups.create("antes do setup", None, &files, vec![]).map_err(|e| e.to_string())?;
    let spec = CmdSpec::new(&cli.exe, &[]).args_owned(args).timeout(Duration::from_secs(20 * 60));
    let op = state.audit.start("setup inicial", "alteracao", None, None);
    let out = runner::run_streaming(&spec, Some(sink(&app, &op_id)), None).await.map_err(|e| e.to_string())?;
    let mut op = op;
    op.backup_id = Some(set.id);
    state.audit.finish(op, out.ok(), "setup", &format!("$ {}\n{}\n{}", out.command, out.stdout, out.stderr));
    Ok(out)
}

#[tauri::command]
pub async fn upgrade_ai_memory(app: AppHandle, state: State<'_, AppState>, op_id: String) -> R<CmdOutput> {
    let _g = state.lock_mutation()?;
    let (cli, before) = state.cli().await?;
    let mut files = vec![];
    for a in ["claude-code", "codex", "cursor"] {
        for c in ["mcp", "hooks"] {
            files.extend(agents::agent_files(&state.paths, a, c, None));
        }
    }
    files.push(state.data_dir().join("config.toml"));
    files.sort();
    files.dedup();
    let set = state.backups.create("antes do upgrade do ai-memory", None, &files, vec![]).map_err(|e| e.to_string())?;
    let spec = CmdSpec::new(&cli.exe, &["upgrade", "--language", "en"]).timeout(Duration::from_secs(20 * 60));
    let op = state.audit.start("upgrade do ai-memory", "alteracao", None, None);
    let mut out = runner::run_streaming(&spec, Some(sink(&app, &op_id)), None).await.map_err(|e| e.to_string())?;
    let after = aimemory::validate_binary(&cli.exe).await.ok();
    out.stdout.push_str(&format!(
        "\n[AI Memory Manager] versão antes: {} · depois: {}\n[AI Memory Manager] backup das configurações: {}\n",
        before.to_string(),
        after.map(|v| v.to_string()).unwrap_or_else(|| "binário não validou!".into()),
        set.id
    ));
    let mut op = op;
    op.backup_id = Some(set.id);
    state.audit.finish(op, out.ok() && after.is_some(), "upgrade", &format!("$ {}\n{}\n{}", out.command, out.stdout, out.stderr));
    Ok(out)
}

#[tauri::command]
pub async fn plan_uninstall(state: State<'_, AppState>, only: String) -> R<ChangePlan> {
    if !["hooks", "mcp", "instructions", "skills"].contains(&only.as_str()) {
        return Err("Escopo inválido.".into());
    }
    let (cli, _) = state.cli().await?;
    let args = vec!["uninstall".to_string(), "--only".into(), only.clone(), "--language".into(), "en".into()];
    let dry = runner::run(&CmdSpec::new(&cli.exe, &[]).args_owned(args.clone()).timeout(Duration::from_secs(60))).await.map_err(|e| e.to_string())?;
    let mut plan = changes::new_plan(
        &format!("Desinstalar '{only}' de TODOS os agentes"),
        "Usa o desinstalador oficial do ai-memory. ATENÇÃO: ele não tem filtro por agente e atinge todos os agentes detectados. Para um único agente, use as ações do card do agente.",
    );
    plan.destructive = true;
    plan.dry_run_output = Some(format!("{}\n{}", dry.stdout, dry.stderr_meaningful()).trim().into());
    plan.commands = vec![CmdSpec::new(&cli.exe, &[]).args_owned([args.clone(), vec!["--apply".into(), "--yes".into()]].concat()).display()];
    plan.warnings.push("Nunca usa --purge-data: memórias armazenadas não são apagadas.".into());
    let mut files = vec![];
    for a in ["claude-code", "codex", "cursor"] {
        for c in ["mcp", "hooks"] {
            files.extend(agents::agent_files(&state.paths, a, c, None));
        }
    }
    files.sort();
    files.dedup();
    plan.files = files.iter().map(|f| f.to_string_lossy().to_string()).collect();
    Ok(state.store_plan(Planned {
        plan,
        action: Action::Official { exe: cli.exe.clone(), args, apply_flags: vec!["--apply".into(), "--yes".into()], cwd: None, timeout: Duration::from_secs(300), flatten_hooks: None },
        backup_files: files,
        verify: Verify::None,
    }))
}

#[tauri::command]
pub async fn server_health(state: State<'_, AppState>) -> R<Health> {
    Ok(service::health(&state.base_url()).await)
}
