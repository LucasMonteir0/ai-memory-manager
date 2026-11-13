//! Motor de alterações: prévia → confirmação → backup → execução → verificação → rollback.

use crate::backup::{BackupStore, FileDiff, RegistryBackup, write_atomic};
use crate::configedit::{self, DEFAULT_MCP_NAME};
use crate::runner::{self, CmdOutput, CmdSpec};
use crate::sanitize::sanitize;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePlan {
    pub id: String,
    pub title: String,
    pub explanation: String,
    pub agent_id: Option<String>,
    pub project_id: Option<String>,
    pub commands: Vec<String>,
    pub files: Vec<String>,
    pub diffs: Vec<FileDiff>,
    pub dry_run_output: Option<String>,
    pub warnings: Vec<String>,
    pub after_steps: Vec<String>,
    pub destructive: bool,
}

#[derive(Debug, Clone)]
pub enum Action {
    /// Comando oficial do ai-memory; na aplicação adiciona `apply_flags`.
    Official { exe: PathBuf, args: Vec<String>, apply_flags: Vec<String>, cwd: Option<PathBuf>, timeout: Duration },
    /// Edição cirúrgica feita pela GUI (conteúdo antigo esperado → novo).
    WriteFiles { writes: Vec<(PathBuf, Option<String>, String)> },
    Registry { key: String, name: String, new_value: Option<String>, expected_old_present: bool, secret: bool },
    CreateMarker { dir: PathBuf, content: String },
    /// Restaura um backup existente (arquivos + valores de registro não secretos).
    RestoreBackup { id: String },
}

#[derive(Debug, Clone)]
pub enum Verify {
    None,
    JsonMcp { file: PathBuf, present: bool },
    CodexMcp { file: PathBuf, present: bool },
    CodexBridge { file: PathBuf },
    Hooks { file: PathBuf, present: bool },
    RegistryValue { key: String, name: String, present: bool },
    FileExists { file: PathBuf },
}

#[derive(Debug, Clone)]
pub struct Planned {
    pub plan: ChangePlan,
    pub action: Action,
    pub backup_files: Vec<PathBuf>,
    pub verify: Verify,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub success: bool,
    pub summary: String,
    pub backup_id: Option<String>,
    pub rolled_back: bool,
    pub diffs: Vec<FileDiff>,
    pub output: Option<CmdOutput>,
    pub verification: String,
    pub after_steps: Vec<String>,
}

pub fn new_plan(title: &str, explanation: &str) -> ChangePlan {
    ChangePlan {
        id: uuid::Uuid::new_v4().to_string(),
        title: title.into(),
        explanation: explanation.into(),
        agent_id: None,
        project_id: None,
        commands: vec![],
        files: vec![],
        diffs: vec![],
        dry_run_output: None,
        warnings: vec![],
        after_steps: vec![],
        destructive: false,
    }
}

// ---------------------------------------------------------------- verificação

pub fn verify(v: &Verify) -> Result<String, String> {
    match v {
        Verify::None => Ok("Sem verificação automática aplicável.".into()),
        Verify::JsonMcp { file, present } => {
            let txt = std::fs::read_to_string(file).unwrap_or_default();
            let val = configedit::parse_json(&txt).map_err(|e| format!("Arquivo ficou inválido: {e}"))?;
            let has = configedit::json_mcp_entry(&val, DEFAULT_MCP_NAME).is_some();
            if has == *present {
                Ok(format!("Entrada MCP '{DEFAULT_MCP_NAME}' {} em {}.", if has { "presente" } else { "ausente" }, file.display()))
            } else {
                Err(format!("Esperado MCP {} mas o arquivo diz o contrário.", if *present { "presente" } else { "ausente" }))
            }
        }
        Verify::CodexMcp { file, present } => {
            let txt = std::fs::read_to_string(file).unwrap_or_default();
            let m = configedit::codex_mcp(&txt, DEFAULT_MCP_NAME).map_err(|e| format!("TOML ficou inválido: {e}"))?;
            if m.present == *present {
                Ok(format!("[mcp_servers.ai-memory] {} em {}.", if m.present { "presente" } else { "ausente" }, file.display()))
            } else {
                Err("Estado do MCP no config.toml diferente do esperado.".into())
            }
        }
        Verify::CodexBridge { file } => {
            let txt = std::fs::read_to_string(file).unwrap_or_default();
            let m = configedit::codex_mcp(&txt, DEFAULT_MCP_NAME).map_err(|e| format!("TOML ficou inválido: {e}"))?;
            if m.uses_bridge() && m.bearer_token_env_var.is_none() && m.url.is_none() {
                Ok(format!("[mcp_servers.ai-memory] usa a ponte local (sem variável de token) em {}.", file.display()))
            } else {
                Err("A entrada do Codex não ficou no formato da ponte local.".into())
            }
        }
        Verify::Hooks { file, present } => {
            let txt = std::fs::read_to_string(file).unwrap_or_default();
            let val = configedit::parse_json(&txt).map_err(|e| format!("Arquivo ficou inválido: {e}"))?;
            let n = configedit::scan_hooks(&val).events.len();
            if (n > 0) == *present {
                Ok(format!("{n} evento(s) com hooks ai-memory em {}.", file.display()))
            } else {
                Err(format!("Esperado hooks {} mas foram encontrados {n} evento(s).", if *present { "presentes" } else { "ausentes" }))
            }
        }
        Verify::RegistryValue { key, name, present } => {
            let has = crate::service::reg_get(key, name).is_some();
            if has == *present {
                Ok(format!("Valor de registro {name} {}.", if has { "presente" } else { "ausente" }))
            } else {
                Err("Valor de registro diferente do esperado.".into())
            }
        }
        Verify::FileExists { file } => {
            if file.is_file() { Ok(format!("{} criado.", file.display())) } else { Err("Arquivo não foi criado.".into()) }
        }
    }
}

/// Indica se a saída do comando contém avisos que impedem considerar sucesso só pelo exit code.
pub fn output_warnings(out: &CmdOutput) -> Vec<String> {
    let text = format!("{}\n{}", out.stdout, out.stderr_meaningful());
    text.lines()
        .filter(|l| {
            let t = l.to_lowercase();
            (t.contains("error") || t.contains("warn") || t.contains("refus") || t.contains("failed") || t.contains("erro"))
                && !t.contains(" info ")
        })
        .map(|l| sanitize(l.trim()))
        .take(10)
        .collect()
}

// ---------------------------------------------------------------- aplicação

pub async fn apply(p: &Planned, backups: &BackupStore) -> ApplyResult {
    // 1) backup
    let registry = match &p.action {
        Action::Registry { key, name, secret, .. } => vec![RegistryBackup {
            key: key.clone(),
            name: name.clone(),
            // valores secretos nunca são gravados no backup
            value: if *secret { None } else { crate::service::reg_get(key, name) },
            secret: *secret,
        }],
        _ => vec![],
    };
    let set = match backups.create(&p.plan.title, p.plan.agent_id.as_deref(), &p.backup_files, registry) {
        Ok(s) => s,
        Err(e) => {
            return ApplyResult {
                success: false,
                summary: format!("Backup falhou; nada foi alterado: {e}"),
                backup_id: None,
                rolled_back: false,
                diffs: vec![],
                output: None,
                verification: String::new(),
                after_steps: vec![],
            };
        }
    };
    let mut output = None;
    // 2) execução
    let exec: Result<(), String> = match &p.action {
        Action::Official { exe, args, apply_flags, cwd, timeout } => {
            let mut all = args.clone();
            all.extend(apply_flags.iter().cloned());
            let mut spec = CmdSpec::new(exe, &[]).args_owned(all).timeout(*timeout);
            if let Some(c) = cwd {
                spec = spec.cwd(c);
            }
            match runner::run(&spec).await {
                Ok(o) => {
                    let ok = o.ok();
                    let code = o.exit_code;
                    output = Some(o);
                    if ok { Ok(()) } else { Err(format!("Comando terminou com código {code:?}.")) }
                }
                Err(e) => Err(format!("Falha ao executar: {e}")),
            }
        }
        Action::WriteFiles { writes } => {
            let mut r = Ok(());
            for (path, expected_old, new) in writes {
                let current = std::fs::read_to_string(path).ok();
                if &current != expected_old {
                    r = Err(format!("{} mudou desde a prévia; operação abortada.", path.display()));
                    break;
                }
                if let Err(e) = write_atomic(path, new.as_bytes()) {
                    r = Err(format!("Falha ao gravar {}: {e}", path.display()));
                    break;
                }
            }
            r
        }
        Action::Registry { key, name, new_value, expected_old_present, .. } => {
            let present = crate::service::reg_get(key, name).is_some();
            if present != *expected_old_present {
                Err("O registro mudou desde a prévia; operação abortada.".into())
            } else {
                crate::service::reg_set(key, name, new_value.as_deref()).map_err(|e| e.to_string())
            }
        }
        Action::CreateMarker { dir, content } => crate::projects::write_marker(dir, content).map(|_| ()),
        Action::RestoreBackup { id } => match backups.get(id) {
            None => Err("Backup não encontrado.".into()),
            Some(b) => {
                let r = backups.restore_files(&b).map(|_| ()).map_err(|e| e.to_string());
                for rb in &b.registry {
                    // secreto: a GUI só cria quando não existia → restaurar = remover
                    let _ = crate::service::reg_set(&rb.key, &rb.name, rb.value.as_deref());
                }
                r
            }
        },
    };
    // 3) verificação
    let ver = match &exec {
        Ok(()) => verify(&p.verify),
        Err(e) => Err(e.clone()),
    };
    let mut warnings = output.as_ref().map(output_warnings).unwrap_or_default();
    // 4) rollback em falha
    let mut rolled_back = false;
    let diffs_before_rollback = backups.diff_against_current(&set);
    if let Err(e) = &ver {
        let restore = backups.restore_files(&set);
        if let Action::Registry { key, name, .. } = &p.action {
            if let Some(rb) = set.registry.first() {
                let _ = crate::service::reg_set(key, name, rb.value.as_deref());
            }
        }
        rolled_back = restore.is_ok();
        warnings.insert(0, format!("Falha: {e}"));
    }
    let success = ver.is_ok();
    ApplyResult {
        success,
        summary: if success {
            if warnings.is_empty() {
                format!("{}: concluído e verificado.", p.plan.title)
            } else {
                format!("{}: concluído com avisos — revise a saída.", p.plan.title)
            }
        } else if rolled_back {
            format!("{}: falhou e foi revertido a partir do backup.", p.plan.title)
        } else {
            format!("{}: falhou; verifique o backup {}.", p.plan.title, set.id)
        },
        backup_id: Some(set.id.clone()),
        rolled_back,
        diffs: if rolled_back { vec![] } else { diffs_before_rollback.into_iter().filter(|d| d.changed).collect() },
        output,
        verification: match ver {
            Ok(s) => s,
            Err(e) => e,
        } + &if warnings.is_empty() { String::new() } else { format!("\nAvisos:\n- {}", warnings.join("\n- ")) },
        after_steps: if success { p.plan.after_steps.clone() } else { vec![] },
    }
}

// ---------------------------------------------------------------- construtores de planos

/// Prévia de remoção cirúrgica do MCP ai-memory (JSON ou TOML do Codex).
pub fn plan_disable_mcp(agent: &str, file: &Path) -> Result<Planned, String> {
    let old = std::fs::read_to_string(file).map_err(|_| "Arquivo de configuração não existe.".to_string())?;
    let (new, verify) = if agent == "codex" {
        let (n, removed) = configedit::remove_codex_mcp(&old, DEFAULT_MCP_NAME).map_err(|e| e.to_string())?;
        if !removed {
            return Err("Não há [mcp_servers.ai-memory] para remover.".into());
        }
        (n, Verify::CodexMcp { file: file.to_path_buf(), present: false })
    } else {
        let v = configedit::parse_json(&old).map_err(|e| e.to_string())?;
        let (v2, removed) = configedit::remove_json_mcp(v, DEFAULT_MCP_NAME).map_err(|e| e.to_string())?;
        if !removed {
            return Err("Não há entrada MCP 'ai-memory' para remover.".into());
        }
        (configedit::to_pretty_json(&v2), Verify::JsonMcp { file: file.to_path_buf(), present: false })
    };
    let mut plan = new_plan(
        &format!("Desconectar MCP — {agent}"),
        "Remove somente a entrada 'ai-memory' deste agente. Outros servidores MCP e os demais agentes não são alterados. A memória armazenada não é apagada.",
    );
    plan.agent_id = Some(agent.into());
    plan.files = vec![file.to_string_lossy().to_string()];
    plan.diffs = vec![FileDiff::new(&file.to_string_lossy(), &old, &new, true)];
    plan.after_steps = vec!["Reinicie o aplicativo do agente para que ele deixe de carregar o MCP.".into()];
    Ok(Planned {
        plan,
        action: Action::WriteFiles { writes: vec![(file.to_path_buf(), Some(old), new)] },
        backup_files: vec![file.to_path_buf()],
        verify,
    })
}

/// Q16 (opção recomendada): troca o MCP do Codex de HTTP+variável de token para a ponte stdio
/// local, que autentica pelo diretório de dados do ai-memory. Nenhum segredo é gravado.
pub fn plan_codex_bridge(file: &Path, exe: &Path, mcp_url: &str) -> Result<Planned, String> {
    let old = std::fs::read_to_string(file).map_err(|_| "config.toml do Codex não encontrado.".to_string())?;
    let cur = configedit::codex_mcp(&old, DEFAULT_MCP_NAME).map_err(|e| e.to_string())?;
    if !cur.present {
        return Err("Não há [mcp_servers.ai-memory] no Codex; use 'Configurar MCP' primeiro.".into());
    }
    if cur.uses_bridge() {
        return Err("O MCP do Codex já usa a ponte local.".into());
    }
    let new = configedit::codex_mcp_to_bridge(&old, DEFAULT_MCP_NAME, &exe.to_string_lossy(), mcp_url).map_err(|e| e.to_string())?;
    let mut plan = new_plan(
        "Codex: usar a ponte local do ai-memory no MCP",
        "Troca a conexão HTTP que depende da variável AI_MEMORY_AUTH_TOKEN pela ponte stdio `ai-memory.exe mcp-bridge --no-session-id`, que lê o token sozinha do diretório de dados do ai-memory (como o Claude Code já faz). Nenhum segredo é gravado em variável ou arquivo; aprovações por ferramenta e outros servidores MCP são preservados.",
    );
    plan.agent_id = Some("codex".into());
    plan.files = vec![file.to_string_lossy().to_string()];
    plan.diffs = vec![FileDiff::new(&file.to_string_lossy(), &old, &new, true)];
    plan.warnings = vec![
        "Este formato não é o que `install-mcp --client codex` gera na 3.4.6. Se você usar 'Reparar' no MCP do Codex depois, ele volta para HTTP + variável de token.".into(),
        "Nesta máquina a ponte foi testada antes da prévia (initialize + tools/list sem a variável de token).".into(),
    ];
    plan.after_steps = vec![
        "Feche e reabra o Codex Desktop para carregar o novo servidor MCP.".into(),
        "Numa sessão nova do Codex, peça algo como \"use memory_status\" para confirmar pelo lado do agente.".into(),
    ];
    Ok(Planned {
        plan,
        action: Action::WriteFiles { writes: vec![(file.to_path_buf(), Some(old), new)] },
        backup_files: vec![file.to_path_buf()],
        verify: Verify::CodexBridge { file: file.to_path_buf() },
    })
}

/// Prévia de remoção cirúrgica dos hooks ai-memory de um único agente.
pub fn plan_disable_hooks(agent: &str, file: &Path) -> Result<Planned, String> {
    let old = std::fs::read_to_string(file).map_err(|_| "Arquivo de hooks não existe.".to_string())?;
    let v = configedit::parse_json(&old).map_err(|e| e.to_string())?;
    let (v2, n) = configedit::remove_ai_memory_hooks(v).map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Nenhum hook ai-memory encontrado neste arquivo.".into());
    }
    let new = configedit::to_pretty_json(&v2);
    let mut plan = new_plan(
        &format!("Desativar captura (hooks) — {agent}"),
        &format!("Remove {n} hook(s) do ai-memory apenas deste agente. Hooks de terceiros e de outros agentes são preservados. Memórias já armazenadas não são apagadas."),
    );
    plan.agent_id = Some(agent.into());
    plan.files = vec![file.to_string_lossy().to_string()];
    plan.diffs = vec![FileDiff::new(&file.to_string_lossy(), &old, &new, true)];
    plan.warnings.push("Eventos já enviados ao spool ainda podem ser entregues ao servidor.".into());
    plan.after_steps = vec!["Reinicie o aplicativo do agente. Novas sessões deixarão de ser capturadas.".into()];
    Ok(Planned {
        plan,
        action: Action::WriteFiles { writes: vec![(file.to_path_buf(), Some(old), new)] },
        backup_files: vec![file.to_path_buf()],
        verify: Verify::Hooks { file: file.to_path_buf(), present: false },
    })
}

/// Prévia de instalação oficial: roda o dry-run e, para MCP, simula o --apply numa cópia
/// temporária (sem efeitos colaterais) para gerar o diff real.
pub async fn plan_official_install(
    exe: &Path,
    agent: &str,
    component: &str,
    client_id: &str,
    config_file: &Path,
    extra_backup: Vec<PathBuf>,
) -> Result<Planned, String> {
    let base_args: Vec<String> = if component == "mcp" {
        vec!["install-mcp".into(), "--client".into(), client_id.into(), "--config-file".into(), config_file.to_string_lossy().to_string(), "--language".into(), "en".into()]
    } else {
        vec!["install-hooks".into(), "--agent".into(), client_id.into(), "--config-file".into(), config_file.to_string_lossy().to_string(), "--language".into(), "en".into()]
    };
    let dry = runner::run(&CmdSpec::new(exe, &[]).args_owned(base_args.clone()).timeout(Duration::from_secs(60)))
        .await
        .map_err(|e| format!("Falha no dry-run: {e}"))?;
    if !dry.ok() {
        return Err(format!("Dry-run falhou (código {:?}): {}", dry.exit_code, dry.stderr_meaningful()));
    }
    let mut plan = new_plan(
        &format!("Configurar {} — {agent}", if component == "mcp" { "MCP" } else { "hooks" }),
        if component == "mcp" {
            "Registra o servidor MCP 'ai-memory' no arquivo do agente usando o instalador oficial (install-mcp --apply). Outros servidores são preservados."
        } else {
            "Registra os hooks de captura do ai-memory usando o instalador oficial (install-hooks --apply). Hooks de terceiros são preservados."
        },
    );
    plan.agent_id = Some(agent.into());
    plan.dry_run_output = Some(format!("{}\n{}", dry.stdout, dry.stderr_meaningful()).trim().to_string());
    plan.commands.push(CmdSpec::new(exe, &[]).args_owned([base_args.clone(), vec!["--apply".into()]].concat()).display());
    let mut files = vec![config_file.to_path_buf()];
    files.extend(extra_backup);
    plan.files = files.iter().map(|f| f.to_string_lossy().to_string()).collect();
    plan.warnings.push("O dry-run mostra a proposta; ele não comprova que a integração está ativa.".into());
    if component == "mcp" {
        // simulação em cópia temporária
        let tmp = std::env::temp_dir().join(format!("aimm-preview-{}", &uuid::Uuid::new_v4().to_string()[..8]));
        let _ = std::fs::create_dir_all(&tmp);
        let tmp_file = tmp.join(config_file.file_name().unwrap_or_default());
        let old = std::fs::read_to_string(config_file).ok();
        if let Some(o) = &old {
            let _ = std::fs::write(&tmp_file, o);
        }
        let mut sim_args = base_args.clone();
        let idx = sim_args.iter().position(|a| a == "--config-file").unwrap() + 1;
        sim_args[idx] = tmp_file.to_string_lossy().to_string();
        sim_args.push("--apply".into());
        if let Ok(o) = runner::run(&CmdSpec::new(exe, &[]).args_owned(sim_args).timeout(Duration::from_secs(60))).await {
            if o.ok() {
                let new = std::fs::read_to_string(&tmp_file).unwrap_or_default();
                plan.diffs.push(FileDiff::new(&config_file.to_string_lossy(), old.as_deref().unwrap_or(""), &new, old.is_some()));
            } else {
                plan.warnings.push("Não foi possível simular o diff; ele será exibido após aplicar.".into());
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
    } else {
        plan.warnings.push("O diff exato dos hooks é exibido após aplicar (a simulação geraria efeitos no diretório de dados).".into());
    }
    if agent == "codex" && component == "hooks" {
        plan.after_steps.push("Abra o Codex Desktop e aprove os hooks quando solicitado (o hash muda a cada instalação).".into());
    }
    if agent == "cursor" && component == "hooks" {
        plan.warnings.push("Experimental no Windows: hooks .sh executados via Git Bash.".into());
    }
    let verify = match (agent, component) {
        ("codex", "mcp") => Verify::CodexMcp { file: config_file.to_path_buf(), present: true },
        (_, "mcp") => Verify::JsonMcp { file: config_file.to_path_buf(), present: true },
        (_, _) => Verify::Hooks { file: config_file.to_path_buf(), present: true },
    };
    plan.after_steps.push("Reinicie o aplicativo do agente e use 'Verificar captura' com uma sessão nova.".into());
    Ok(Planned {
        plan,
        action: Action::Official {
            exe: exe.to_path_buf(),
            args: base_args,
            apply_flags: vec!["--apply".into()],
            cwd: None,
            timeout: Duration::from_secs(120),
        },
        backup_files: files,
        verify,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disable_hooks_only_touches_target_and_rolls_back_on_verify_failure() {
        let d = tempfile::tempdir().unwrap();
        let store = BackupStore::new(d.path());
        let f = d.path().join("settings.json");
        let other_agent = d.path().join("hooks.json");
        std::fs::write(&f, r#"{"hooks":{"Stop":[{"hooks":[{"command":"C:\\x\\ai-memory.exe","args":["hook","--event","stop"]},{"command":"echo"}]}]},"keep":1}"#).unwrap();
        std::fs::write(&other_agent, r#"{"hooks":{"Stop":[{"hooks":[{"command":"ai-memory.exe hook --event stop --agent codex"}]}]}}"#).unwrap();
        let p = plan_disable_hooks("claude-code", &f).unwrap();
        assert_eq!(p.plan.diffs.len(), 1);
        let r = apply(&p, &store).await;
        assert!(r.success, "{r:?}");
        let after = std::fs::read_to_string(&f).unwrap();
        assert!(!after.contains("ai-memory") && after.contains("echo") && after.contains("\"keep\""));
        // outro agente intacto
        assert!(std::fs::read_to_string(&other_agent).unwrap().contains("ai-memory"));
        assert!(r.backup_id.is_some());
    }

    #[tokio::test]
    async fn aborts_if_file_changed_since_preview() {
        let d = tempfile::tempdir().unwrap();
        let store = BackupStore::new(d.path());
        let f = d.path().join("mcp.json");
        std::fs::write(&f, r#"{"mcpServers":{"ai-memory":{"command":"x"},"o":{"command":"y"}}}"#).unwrap();
        let p = plan_disable_mcp("cursor", &f).unwrap();
        std::fs::write(&f, r#"{"mcpServers":{"ai-memory":{"command":"x"},"o":{"command":"y"},"novo":{}}}"#).unwrap();
        let r = apply(&p, &store).await;
        assert!(!r.success);
        assert!(std::fs::read_to_string(&f).unwrap().contains("novo"));
    }

    #[tokio::test]
    async fn rollback_after_partial_failure_of_official_command() {
        // Simula um instalador que altera o arquivo e falha (exit 1): deve restaurar.
        let d = tempfile::tempdir().unwrap();
        let store = BackupStore::new(d.path());
        let f = d.path().join("mcp.json");
        std::fs::write(&f, "{\"mcpServers\":{}}").unwrap();
        let cmd = std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into());
        let script = format!("echo {{\"broken\": > \"{}\" & exit /b 1", f.display());
        let planned = Planned {
            plan: new_plan("teste", "x"),
            action: Action::Official {
                exe: cmd.into(),
                args: vec!["/d".into(), "/c".into(), script],
                apply_flags: vec![],
                cwd: None,
                timeout: Duration::from_secs(10),
            },
            backup_files: vec![f.clone()],
            verify: Verify::JsonMcp { file: f.clone(), present: true },
        };
        let r = apply(&planned, &store).await;
        assert!(!r.success && r.rolled_back, "{r:?}");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "{\"mcpServers\":{}}");
    }

    #[tokio::test]
    async fn verify_failure_rolls_back_even_with_exit_zero() {
        let d = tempfile::tempdir().unwrap();
        let store = BackupStore::new(d.path());
        let f = d.path().join("mcp.json");
        std::fs::write(&f, "{}").unwrap();
        let cmd = std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into());
        let planned = Planned {
            plan: new_plan("teste", "x"),
            action: Action::Official { exe: cmd.into(), args: vec!["/d".into(), "/c".into(), "exit /b 0".into()], apply_flags: vec![], cwd: None, timeout: Duration::from_secs(10) },
            backup_files: vec![f.clone()],
            verify: Verify::JsonMcp { file: f.clone(), present: true },
        };
        let r = apply(&planned, &store).await;
        assert!(!r.success, "exit 0 sem a entrada não pode ser sucesso");
    }

    #[test]
    fn warnings_detected_in_output() {
        let o = CmdOutput {
            command: "x".into(), cwd: None, exit_code: Some(0),
            stdout: "ok\nwarning: hooks dir not found\n".into(), stderr: String::new(),
            timed_out: false, cancelled: false, duration_ms: 1,
        };
        assert_eq!(output_warnings(&o).len(), 1);
    }
}
