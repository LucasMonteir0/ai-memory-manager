//! Testes SOMENTE LEITURA contra a instalação real do ai-memory desta máquina.
//! Ignorados por padrão. Rodar com:
//!   cargo test --test real_readonly -- --ignored --nocapture
//! Variável opcional AIMM_REAL_PROJECT=<pasta> (obrigatória).
//! Nenhum comando com --apply, nenhuma escrita em configs de agentes.

use ai_memory_manager_lib::{agents, aimemory, changes, paths::SysPaths, sanitize, service};
use std::path::PathBuf;
use std::time::Duration;

fn project_dir() -> PathBuf {
    std::env::var_os("AIMM_REAL_PROJECT").map(PathBuf::from).expect("defina AIMM_REAL_PROJECT=<pasta de um projeto com sessões>")
}

#[tokio::test]
#[ignore]
async fn real_installation_readonly() {
    let paths = SysPaths::from_env();
    // 1) locator
    let (exe, ver) = {
        let mut found = None;
        for (p, src) in aimemory::candidate_paths(&paths, None) {
            if let Ok(v) = aimemory::validate_binary(&p).await {
                println!("binário: {} [{src}] v{}", p.display(), v.to_string());
                found = Some((p, v));
                break;
            }
        }
        found.expect("ai-memory instalado")
    };
    assert_eq!(aimemory::version_support(ver), "supported");
    let cli = aimemory::AiMemoryCli { exe: exe.clone() };

    // 2) help -> subcomandos
    let help = cli.help(None).await.unwrap();
    let cmds = aimemory::parse_help_commands(&help.stdout);
    println!("subcomandos ({}): recover={} hook-drain={}", cmds.len(), cmds.contains("recover"), cmds.contains("hook-drain"));
    assert!(cmds.contains("status") && cmds.contains("doctor"));

    // 3) config, token (nunca impresso), health
    let dd = paths.ai_memory_default_data_dir();
    let cfg = service::read_ai_memory_config(&dd);
    let base = service::server_base_url(&cfg);
    let token = service::read_token(&dd, &cfg);
    println!("server: {base} · token encontrado: {}", token.is_some());
    let h = service::health(&base).await;
    println!("healthz: {:?} {:?}ms", h.http_status, h.latency_ms);
    assert!(h.reachable, "servidor deve estar online para este teste");

    // 4) status --json
    let st = cli.run(&["status", "--json"], None, Duration::from_secs(30)).await.unwrap();
    let parsed = aimemory::parse_status(&st.stdout);
    match &parsed {
        aimemory::Interpreted::Ok { data } => println!(
            "status: pages={:?} sessions={:?} obs={:?} spool={:?} emb={:?} auth={:?}",
            data.pages, data.sessions, data.observations, data.spool_pending, data.embedding_status, data.auth_posture
        ),
        aimemory::Interpreted::Raw { reason, .. } => panic!("status não interpretado: {reason}"),
    }
    if let Some(t) = &token {
        assert!(!st.stdout.contains(t.as_str()) && !st.stderr.contains(t.as_str()), "token vazou na saída sanitizada");
    }

    // 5) doctor com cwd do projeto == doctor do CLI
    let dir = project_dir();
    let d1 = cli.run(&["doctor", "--json", "--language", "en"], Some(&dir), Duration::from_secs(90)).await.unwrap();
    let aimemory::Interpreted::Ok { data: doc } = aimemory::parse_doctor(&d1.stdout) else { panic!("doctor não interpretado") };
    println!("doctor {}: {}/{} marker={:?} rows={:?}", dir.display(), doc.workspace, doc.project, doc.marker_status, doc.rows.iter().map(|r| (r.agent.clone(), r.local_total, r.captured)).collect::<Vec<_>>());
    println!("codex hooks trusted={:?} untrusted={:?}", doc.codex_hooks_trusted, doc.codex_hooks_untrusted);
    assert_eq!(doc.project, dir.file_name().unwrap().to_string_lossy().to_lowercase());

    // 6) MCP: com e sem token
    let with = service::test_mcp(&base, token.as_deref()).await;
    let without = service::test_mcp(&base, None).await;
    println!("MCP com token: ok={} tools={:?} err={:?}", with.ok, with.tool_count, with.error);
    println!("MCP sem token: ok={} http={:?}", without.ok, without.http_status);
    assert!(with.ok);

    // 7) agentes reais
    let apps = agents::detect_desktop_apps(&paths).await;
    println!("apps: claude={:?}/{:?} codex={:?}/{:?} cursor={:?} node={} gitbash={:?}",
        apps.claude.installed, apps.claude.version, apps.codex.installed, apps.codex.version, apps.cursor.installed, apps.node_available, apps.git_bash);
    let ctx = agents::Ctx { paths: &paths, apps: &apps, doctor: Some(&doc), server_requires_token: token.is_some() };
    for r in agents::all_reports(&ctx) {
        let line = format!("{:<24} MCP={:?} ({}) | hooks={:?} ({})", r.display_name, r.mcp.status, r.mcp.detail, r.hooks.status, r.hooks.detail);
        println!("{}", sanitize::sanitize(&line));
    }

    // 8) prévias (não aplicadas) de desativação cirúrgica nos arquivos reais
    if paths.claude_settings().is_file() {
        let p = changes::plan_disable_hooks("claude-code", &paths.claude_settings()).unwrap();
        let d = &p.plan.diffs[0];
        println!("prévia desativar hooks Claude Code: -{} +{} linhas (NÃO aplicada)", d.removed, d.added);
    }
    let codex_cfg = paths.codex_home().join("config.toml");
    if codex_cfg.is_file() {
        if let Ok(p) = changes::plan_disable_mcp("codex", &codex_cfg) {
            let d = &p.plan.diffs[0];
            println!("prévia desconectar MCP Codex: -{} +{} linhas (NÃO aplicada); outros servidores preservados", d.removed, d.added);
            assert!(!d.unified.contains("node_repl") || d.unified.lines().filter(|l| l.starts_with('-') && l.contains("node_repl")).count() == 0);
        }
    }

    // 8b) ponte stdio sem variável de token + prévia (não aplicada) da migração do Codex
    let bridge = service::test_mcp_bridge(&exe, &format!("{base}/mcp")).await;
    println!("ponte stdio: ok={} tools={:?} err={:?}", bridge.ok, bridge.tool_count, bridge.error);
    assert!(bridge.ok);
    if codex_cfg.is_file() {
        match changes::plan_codex_bridge(&codex_cfg, &exe, &format!("{base}/mcp")) {
            Ok(p) => {
                let d = &p.plan.diffs[0];
                println!("prévia Codex→ponte: -{} +{} (NÃO aplicada)\n{}", d.removed, d.added, d.unified);
            }
            Err(e) => println!("prévia Codex→ponte: {e}"),
        }
    }

    // 9) dry-runs oficiais (sem --apply)
    let bf = cli.run(&["backfill", "--dry-run", "--json"], Some(&dir), Duration::from_secs(120)).await.unwrap();
    println!("backfill --dry-run: {:?}", aimemory::parse_backfill(&bf.stdout));
}
