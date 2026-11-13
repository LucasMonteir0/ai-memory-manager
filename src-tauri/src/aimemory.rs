//! Localização/validação do binário ai-memory e interpretação das saídas `--json`.
//! Toda interpretação é tolerante: se o formato mudar, retorna `Interpreted::Raw`
//! (estado "Não interpretado") — nunca assume "saudável".

use crate::paths::SysPaths;
use crate::runner::{self, CmdOutput, CmdSpec};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------- versão

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Semver(pub u64, pub u64, pub u64);

impl Semver {
    pub fn parse(s: &str) -> Option<Self> {
        static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"v?(\d+)\.(\d+)\.(\d+)").unwrap());
        let c = RE.captures(s)?;
        Some(Semver(c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?))
    }
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Faixa testada por este gerenciador. Fora dela: "versão não suportada/não testada".
pub const MIN_SUPPORTED: Semver = Semver(3, 4, 0);
pub const MAX_TESTED_MAJOR: u64 = 3;

pub fn version_support(v: Semver) -> &'static str {
    if v < MIN_SUPPORTED {
        "unsupported"
    } else if v.0 > MAX_TESTED_MAJOR {
        "untested"
    } else {
        "supported"
    }
}

/// Interpreta a saída de `ai-memory --version`. Exige o nome do programa para não aceitar
/// qualquer executável que imprima um número.
pub fn parse_version_output(out: &str) -> Option<Semver> {
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?mi)^\s*ai-memory(?:\.exe)?\s+v?(\d+\.\d+\.\d+)").unwrap());
    let c = RE.captures(out)?;
    Semver::parse(&c[1])
}

/// Lista de subcomandos a partir de `--help` (seção "Commands:").
pub fn parse_help_commands(help: &str) -> BTreeSet<String> {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s{2,}([a-z][a-z0-9\-]+)\s{2,}\S").unwrap());
    let mut set = BTreeSet::new();
    let mut in_cmds = false;
    for line in help.lines() {
        let t = line.trim_end();
        if t.eq_ignore_ascii_case("Commands:") {
            in_cmds = true;
            continue;
        }
        if in_cmds {
            if t.is_empty() {
                continue;
            }
            if !line.starts_with(' ') {
                in_cmds = false;
                continue;
            }
            if let Some(c) = RE.captures(line) {
                set.insert(c[1].to_string());
            } else if let Some(word) = line.split_whitespace().next() {
                // linha com o nome sozinho (descrição na linha seguinte)
                if word.chars().all(|ch| ch.is_ascii_lowercase() || ch == '-' || ch.is_ascii_digit())
                    && line.trim() == word
                {
                    set.insert(word.to_string());
                }
            }
        }
    }
    set
}

// ---------------------------------------------------------------- locator

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryCandidate {
    pub path: String,
    pub source: String,
    pub valid: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}

pub fn candidate_paths(paths: &SysPaths, manual: Option<&Path>) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    if let Some(m) = manual {
        out.push((m.to_path_buf(), "manual".into()));
    }
    out.push((
        paths.ai_memory_default_install_dir().join("ai-memory.exe"),
        "padrao".into(),
    ));
    if let Some(pathvar) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&pathvar) {
            let p = dir.join("ai-memory.exe");
            if p.is_file() {
                out.push((p, "PATH".into()));
            }
        }
    }
    out.push((paths.home.join(".cargo").join("bin").join("ai-memory.exe"), "cargo".into()));
    // dedup case-insensitive
    let mut seen = BTreeSet::new();
    out.retain(|(p, _)| seen.insert(p.to_string_lossy().to_lowercase()));
    out
}

pub async fn validate_binary(path: &Path) -> Result<Semver, String> {
    if !path.is_file() {
        return Err("arquivo não encontrado".into());
    }
    let spec = CmdSpec::new(path, &["--version"]).timeout(Duration::from_secs(15));
    let out = runner::run(&spec).await.map_err(|e| format!("falha ao executar: {e}"))?;
    if !out.ok() {
        return Err(format!(
            "--version retornou código {:?}: {}",
            out.exit_code,
            out.stderr_meaningful()
        ));
    }
    parse_version_output(&out.stdout)
        .ok_or_else(|| "saída de --version não identifica o ai-memory".to_string())
}

// ---------------------------------------------------------------- JSON helpers

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Interpreted<T> {
    Ok { data: T },
    Raw { raw: String, reason: String },
}

/// Extrai o primeiro objeto JSON de stdout (tolerando texto antes/depois).
pub fn extract_json(stdout: &str) -> Option<Value> {
    let start = stdout.find('{')?;
    let end = stdout.rfind('}')?;
    if end < start {
        return None;
    }
    serde_json::from_str(&stdout[start..=end]).ok()
}

fn u(v: &Value, ptr: &str) -> Option<u64> {
    v.pointer(ptr).and_then(Value::as_u64)
}
fn s(v: &Value, ptr: &str) -> Option<String> {
    v.pointer(ptr).and_then(Value::as_str).map(str::to_string)
}
fn b(v: &Value, ptr: &str) -> Option<bool> {
    v.pointer(ptr).and_then(Value::as_bool)
}

// ---------------------------------------------------------------- status

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub version: Option<String>,
    pub data_dir: Option<String>,
    pub bind: Option<String>,
    pub server_url: Option<String>,
    pub client_auth: Option<bool>,
    pub auth_posture: Option<String>,
    pub auth_fix: Option<String>,
    pub pages: Option<u64>,
    pub sessions: Option<u64>,
    pub observations: Option<u64>,
    pub fts_pages_ok: Option<bool>,
    pub fts_observations_ok: Option<bool>,
    pub embedding_status: Option<String>,
    pub embedding_rows: Option<u64>,
    pub llm_status: Option<String>,
    pub spool_pending: Option<u64>,
    pub spool_oldest_age_ms: Option<u64>,
    pub spool_capacity: Option<u64>,
    pub ingest_accepted: Option<u64>,
    pub ingest_failed: Option<u64>,
    pub ingest_dropped_unauthorized: Option<u64>,
    pub last_persisted_ms: Option<i64>,
    pub capture_mode: Option<String>,
    pub database_bytes: Option<u64>,
    pub consolidation_failed: Option<u64>,
    pub recovery_journal_pending: Option<u64>,
}

pub fn parse_status(stdout: &str) -> Interpreted<Status> {
    let Some(v) = extract_json(stdout) else {
        return Interpreted::Raw {
            raw: stdout.to_string(),
            reason: "saída não contém JSON".into(),
        };
    };
    // exige ao menos counts para considerar interpretado
    if v.pointer("/counts").is_none() {
        return Interpreted::Raw {
            raw: stdout.to_string(),
            reason: "JSON sem o bloco 'counts' esperado".into(),
        };
    }
    let fts_pages = match (u(&v, "/derived/pages_rows"), u(&v, "/derived/pages_fts_rows")) {
        (Some(a), Some(b)) => Some(a == b),
        _ => None,
    };
    let fts_obs = match (
        u(&v, "/derived/observations_rows"),
        u(&v, "/derived/observations_fts_rows"),
    ) {
        (Some(a), Some(b)) => Some(a == b),
        _ => None,
    };
    Interpreted::Ok {
        data: Status {
            version: s(&v, "/version"),
            data_dir: s(&v, "/data_dir").map(|d| crate::paths::display(Path::new(&d))),
            bind: s(&v, "/bind"),
            server_url: s(&v, "/client/server_url"),
            client_auth: b(&v, "/client/auth"),
            auth_posture: s(&v, "/auth_posture/posture"),
            auth_fix: s(&v, "/auth_posture/fix"),
            pages: u(&v, "/counts/pages_latest"),
            sessions: u(&v, "/counts/sessions"),
            observations: u(&v, "/counts/observations"),
            fts_pages_ok: fts_pages,
            fts_observations_ok: fts_obs,
            embedding_status: s(&v, "/providers/embedding/status"),
            embedding_rows: u(&v, "/derived/embedding_rows"),
            llm_status: s(&v, "/providers/llm/status"),
            spool_pending: u(&v, "/spool/pending"),
            spool_oldest_age_ms: u(&v, "/spool/oldest_age_ms"),
            spool_capacity: u(&v, "/spool/capacity"),
            ingest_accepted: u(&v, "/ingest/accepted"),
            ingest_failed: u(&v, "/ingest/failed"),
            ingest_dropped_unauthorized: u(&v, "/ingest/dropped_unauthorized"),
            last_persisted_ms: v.pointer("/ingest/last_persisted_ms").and_then(Value::as_i64),
            capture_mode: s(&v, "/capture_mode"),
            database_bytes: u(&v, "/storage/database_bytes"),
            consolidation_failed: u(&v, "/consolidation/failed"),
            recovery_journal_pending: u(&v, "/recovery/journal_pending"),
        },
    }
}

// ---------------------------------------------------------------- doctor

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorRow {
    pub agent: String,
    pub local_total: u64,
    pub local_recent: u64,
    pub captured: u64,
    pub uncaptured: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Doctor {
    pub workspace: String,
    pub project: String,
    pub server: Option<String>,
    pub since_days: Option<u64>,
    pub rows: Vec<DoctorRow>,
    pub capture_coverage_problem: Option<String>,
    pub marker_capture_problem: Option<String>,
    pub uncaptured_agents: Vec<String>,
    pub marker_status: Option<String>,
    pub scope_source: Option<String>,
    pub server_project_status: Option<String>,
    pub identity_level: Option<String>,
    pub server_unreachable: bool,
    pub spool_pending: Option<u64>,
    pub spool_oldest_age_ms: Option<u64>,
    pub codex_hooks_trusted: Vec<String>,
    pub codex_hooks_untrusted: Vec<String>,
    pub plugins: Vec<String>,
    pub native_memory: Vec<NativeMemory>,
    pub auth_posture: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMemory {
    pub agent: String,
    pub file_count: u64,
    pub status: String,
}

fn str_list(v: &Value, ptr: &str) -> Vec<String> {
    v.pointer(ptr)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|x| match x {
                    Value::String(s) => s.clone(),
                    other => other
                        .get("name")
                        .or_else(|| other.get("agent"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| other.to_string()),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_doctor(stdout: &str) -> Interpreted<Doctor> {
    let Some(v) = extract_json(stdout) else {
        return Interpreted::Raw {
            raw: stdout.to_string(),
            reason: "saída não contém JSON".into(),
        };
    };
    let (Some(ws), Some(pr), Some(rows)) = (
        s(&v, "/workspace"),
        s(&v, "/project"),
        v.pointer("/rows").and_then(Value::as_array),
    ) else {
        return Interpreted::Raw {
            raw: stdout.to_string(),
            reason: "JSON sem workspace/project/rows".into(),
        };
    };
    let rows: Vec<DoctorRow> = rows
        .iter()
        .map(|r| DoctorRow {
            agent: r.get("agent").and_then(Value::as_str).unwrap_or("?").to_string(),
            local_total: r.get("local_total").and_then(Value::as_u64).unwrap_or(0),
            local_recent: r.get("local_recent").and_then(Value::as_u64).unwrap_or(0),
            captured: r.get("captured").and_then(Value::as_u64).unwrap_or(0),
            uncaptured: r.get("uncaptured").and_then(Value::as_bool).unwrap_or(false),
        })
        .collect();
    let mut d = Doctor {
        workspace: ws,
        project: pr,
        server: s(&v, "/server"),
        since_days: u(&v, "/since_days"),
        rows,
        capture_coverage_problem: v
            .pointer("/capture_coverage_problem")
            .filter(|x| !x.is_null())
            .map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())),
        marker_capture_problem: v
            .pointer("/marker_capture_problem")
            .filter(|x| !x.is_null())
            .map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())),
        uncaptured_agents: str_list(&v, "/uncaptured"),
        marker_status: s(&v, "/project_coordinate/local/marker_status"),
        scope_source: s(&v, "/project_coordinate/local/scope_source"),
        server_project_status: s(&v, "/project_coordinate/server/status"),
        identity_level: s(&v, "/identity/level"),
        server_unreachable: b(&v, "/server_unreachable").unwrap_or(false),
        spool_pending: u(&v, "/spool/pending"),
        spool_oldest_age_ms: u(&v, "/spool/oldest_age_ms"),
        codex_hooks_trusted: str_list(&v, "/codex_hooks/trusted"),
        codex_hooks_untrusted: str_list(&v, "/codex_hooks/skipped"),
        plugins: str_list(&v, "/plugins"),
        native_memory: v
            .pointer("/native_memory")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|m| NativeMemory {
                        agent: m.get("agent").and_then(Value::as_str).unwrap_or("?").into(),
                        file_count: m.get("file_count").and_then(Value::as_u64).unwrap_or(0),
                        status: m.get("status").and_then(Value::as_str).unwrap_or("").into(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        auth_posture: v
            .pointer("/auth_posture")
            .and_then(|x| x.as_str().map(str::to_string).or_else(|| s(x, "/posture"))),
        warnings: vec![],
    };
    d.warnings = doctor_warnings(&d);
    Interpreted::Ok { data: d }
}

pub fn doctor_warnings(d: &Doctor) -> Vec<String> {
    let mut w = Vec::new();
    if d.server_unreachable {
        w.push("Servidor inacessível durante o doctor.".into());
    }
    if let Some(p) = &d.capture_coverage_problem {
        w.push(format!("Cobertura de captura: {p}"));
    }
    if let Some(p) = &d.marker_capture_problem {
        w.push(format!("Marker: {p}"));
    }
    for a in &d.uncaptured_agents {
        w.push(format!("{a}: há sessões locais nunca capturadas."));
    }
    if !d.codex_hooks_untrusted.is_empty() {
        w.push(format!(
            "Codex: hooks não autorizados ({}). Aprove-os no Codex Desktop.",
            d.codex_hooks_untrusted.join(", ")
        ));
    }
    if let Some(age) = d.spool_oldest_age_ms {
        if age > 10 * 60 * 1000 && d.spool_pending.unwrap_or(0) > 0 {
            w.push(format!(
                "Spool atrasado: {} evento(s) pendente(s), o mais antigo há {} min.",
                d.spool_pending.unwrap_or(0),
                age / 60000
            ));
        }
    }
    if d.server_project_status.as_deref() == Some("missing") {
        w.push("O servidor ainda não tem dados deste projeto.".into());
    }
    w
}

// ---------------------------------------------------------------- backfill

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backfill {
    pub workspace: Option<String>,
    pub project: Option<String>,
    pub selected: u64,
    pub imported_sessions: u64,
    pub imported_events: u64,
    pub skipped_for_cap: u64,
    pub failed_sessions: u64,
    pub skipped_non_empty: bool,
    pub skipped_not_admitted: bool,
    pub dry_run: bool,
}

pub fn parse_backfill(stdout: &str) -> Interpreted<Backfill> {
    let Some(v) = extract_json(stdout) else {
        return Interpreted::Raw {
            raw: stdout.to_string(),
            reason: "saída não contém JSON".into(),
        };
    };
    if v.get("selected").is_none() || v.get("dry_run").is_none() {
        return Interpreted::Raw {
            raw: stdout.to_string(),
            reason: "JSON sem 'selected'/'dry_run'".into(),
        };
    }
    Interpreted::Ok {
        data: Backfill {
            workspace: s(&v, "/workspace"),
            project: s(&v, "/project"),
            selected: u(&v, "/selected").unwrap_or(0),
            imported_sessions: u(&v, "/imported_sessions").unwrap_or(0),
            imported_events: u(&v, "/imported_events").unwrap_or(0),
            skipped_for_cap: u(&v, "/skipped_for_cap").unwrap_or(0),
            failed_sessions: u(&v, "/failed_sessions").unwrap_or(0),
            skipped_non_empty: b(&v, "/skipped_non_empty").unwrap_or(false),
            skipped_not_admitted: b(&v, "/skipped_not_admitted").unwrap_or(false),
            dry_run: b(&v, "/dry_run").unwrap_or(false),
        },
    }
}

// ---------------------------------------------------------------- setup --plan

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlanAgent {
    pub agent: String,
    pub found_kind: Option<String>,
    pub found_path: Option<String>,
    pub wiring: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlan {
    pub fresh: bool,
    pub data_dir: Option<String>,
    pub config_path: Option<String>,
    pub service_action: Option<String>,
    pub service_manager: Option<String>,
    pub service_path: Option<String>,
    pub agents: Vec<SetupPlanAgent>,
    pub other_agents: Vec<String>,
}

pub fn parse_setup_plan(stdout: &str) -> Interpreted<SetupPlan> {
    let Some(v) = extract_json(stdout) else {
        return Interpreted::Raw {
            raw: stdout.to_string(),
            reason: "saída não contém JSON".into(),
        };
    };
    if v.get("agents").is_none() {
        return Interpreted::Raw {
            raw: stdout.to_string(),
            reason: "JSON sem 'agents'".into(),
        };
    }
    Interpreted::Ok {
        data: SetupPlan {
            fresh: b(&v, "/fresh").unwrap_or(false),
            data_dir: s(&v, "/data_dir").map(|d| crate::paths::display(Path::new(&d))),
            config_path: s(&v, "/config_path").map(|d| crate::paths::display(Path::new(&d))),
            service_action: s(&v, "/service/action"),
            service_manager: s(&v, "/service/manager"),
            service_path: s(&v, "/service/path"),
            agents: v
                .get("agents")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|x| SetupPlanAgent {
                            agent: s(x, "/agent").unwrap_or_default(),
                            found_kind: s(x, "/found/kind"),
                            found_path: s(x, "/found/path"),
                            wiring: s(x, "/wiring"),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            other_agents: str_list(&v, "/other_agents"),
        },
    }
}

// ---------------------------------------------------------------- CLI wrapper

/// Wrapper que sempre usa o caminho exato do binário (nunca PATH implícito).
#[derive(Debug, Clone)]
pub struct AiMemoryCli {
    pub exe: PathBuf,
}

impl AiMemoryCli {
    pub fn spec(&self, args: &[&str]) -> CmdSpec {
        CmdSpec::new(&self.exe, args)
    }
    pub async fn run(&self, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> std::io::Result<CmdOutput> {
        let mut spec = self.spec(args).timeout(timeout);
        if let Some(c) = cwd {
            spec = spec.cwd(c);
        }
        runner::run(&spec).await
    }
    pub async fn help(&self, sub: Option<&str>) -> std::io::Result<CmdOutput> {
        match sub {
            Some(s) => self.run(&[s, "--help"], None, Duration::from_secs(15)).await,
            None => self.run(&["--help"], None, Duration::from_secs(15)).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name),
        )
        .unwrap()
    }

    #[test]
    fn version_requires_program_name() {
        assert_eq!(parse_version_output("ai-memory 3.4.6\n"), Some(Semver(3, 4, 6)));
        assert_eq!(parse_version_output("ai-memory.exe v3.5.0"), Some(Semver(3, 5, 0)));
        assert_eq!(parse_version_output("node v24.18.0"), None);
        assert_eq!(parse_version_output("3.4.6"), None);
    }

    #[test]
    fn version_support_ranges() {
        assert_eq!(version_support(Semver(3, 4, 6)), "supported");
        assert_eq!(version_support(Semver(3, 3, 9)), "unsupported");
        assert_eq!(version_support(Semver(4, 0, 0)), "untested");
    }

    #[test]
    fn help_commands_parsed() {
        let help = "Usage: ai-memory [OPTIONS] <COMMAND>\n\nCommands:\n  setup          Guided setup\n  status         Show status\n  recover        Recover\n  help           Print this message\n\nOptions:\n  -h, --help  Print help\n";
        let c = parse_help_commands(help);
        assert!(c.contains("setup") && c.contains("recover") && c.contains("status"));
        assert!(!c.contains("hook-drain"));
    }

    #[test]
    fn status_fixture_distinguishes_counts_fts_embeddings() {
        let Interpreted::Ok { data } = parse_status(&fx("status.json")) else { panic!() };
        assert_eq!(data.pages, Some(13));
        assert_eq!(data.sessions, Some(28));
        assert!(data.observations.unwrap() > 0);
        assert_eq!(data.fts_pages_ok, Some(true));
        assert_eq!(data.embedding_status.as_deref(), Some("disabled"));
        assert_eq!(data.auth_posture.as_deref(), Some("token"));
        assert_eq!(data.client_auth, Some(true));
        assert_eq!(data.server_url.as_deref(), Some("http://127.0.0.1:49374"));
    }

    #[test]
    fn status_garbage_is_raw_not_healthy() {
        assert!(matches!(parse_status("Error: connection refused"), Interpreted::Raw { .. }));
        assert!(matches!(parse_status("{\"foo\": 1}"), Interpreted::Raw { .. }));
    }

    #[test]
    fn doctor_fixture_a() {
        let Interpreted::Ok { data } = parse_doctor(&fx("doctor-projeto-a.json")) else { panic!() };
        assert_eq!(data.workspace, "default");
        assert_eq!(data.project, "projeto-a");
        let cc = data.rows.iter().find(|r| r.agent == "claude-code").unwrap();
        assert_eq!((cc.local_total, cc.captured), (31, 22));
        assert_eq!(data.marker_status.as_deref(), Some("absent"));
        assert_eq!(data.codex_hooks_trusted.len(), 7);
        assert!(data.codex_hooks_untrusted.is_empty());
        assert!(data.warnings.is_empty(), "{:?}", data.warnings);
    }

    #[test]
    fn doctor_fixture_b_untrusted_uncaptured_spool() {
        let Interpreted::Ok { data } = parse_doctor(&fx("doctor-projeto-b-untrusted.json")) else { panic!() };
        assert_eq!(data.project, "projeto-b");
        assert_eq!(data.marker_status.as_deref(), Some("present"));
        assert_eq!(data.codex_hooks_untrusted.len(), 5);
        assert_eq!(data.uncaptured_agents, vec!["claude-code".to_string()]);
        assert!(data.warnings.iter().any(|w| w.contains("não autorizados")));
        assert!(data.warnings.iter().any(|w| w.contains("Spool atrasado")));
        assert!(data.warnings.iter().any(|w| w.contains("nunca capturadas")));
    }

    #[test]
    fn doctor_unknown_output_is_raw() {
        assert!(matches!(parse_doctor("doctor: everything fine :)"), Interpreted::Raw { .. }));
    }

    #[test]
    fn backfill_fixtures() {
        let Interpreted::Ok { data } = parse_backfill(&fx("backfill-skipped-non-empty.json")) else { panic!() };
        assert!(data.skipped_non_empty && data.dry_run && data.selected == 0);
        let Interpreted::Ok { data } = parse_backfill(&fx("backfill-dry-run.json")) else { panic!() };
        assert_eq!(data.selected, 25);
        assert!(!data.skipped_non_empty);
    }

    #[test]
    fn setup_plan_fixture() {
        let Interpreted::Ok { data } = parse_setup_plan(&fx("setup-plan.json")) else { panic!() };
        assert!(!data.fresh);
        assert!(data.agents.iter().any(|a| a.agent == "codex"));
        assert!(data.other_agents.contains(&"cursor".to_string()));
    }
}
