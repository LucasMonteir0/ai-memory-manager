# Decisões de projeto (sessão de grilling, 13/11/2026)

Base: `AI_MEMORY_MANAGER_ESPECIFICACAO.md` v1.0. Fatos verificados na máquina de referência (ai-memory 3.4.6).

## Fatos do ambiente que divergem da especificação
- `hook-drain` não existe no 3.4.6 → usar `recover [--json]`.
- `project init` é apenas interativo (sem flags não interativas).
- `uninstall` não tem filtro por agente (`--only hooks|mcp|instructions|skills` atinge todos).
- `setup` suporta `--plan [--json]`, `--yes`, `--agent <A>`, `--no-service`.
- `status --json`, `doctor --json`, `backfill --json` existem.
- `backfill` é no-op quando o banco já possui sessões, salvo `--force`.
- Autostart oficial no Windows: valor `ai-memory` em `HKCU\...\Run` executando `service-host`; parada via arquivo `service-host.stop`.
- Saúde: `GET /healthz` (200). `/health` retorna 404.
- Cursor no Windows nativo: `install-hooks --agent cursor` só propõe scripts `.sh` (experimental).
- Codex MCP usa `bearer_token_env_var = "AI_MEMORY_AUTH_TOKEN"`, variável possivelmente ausente.

## Decisões
| # | Decisão |
|---|---|
| Q1 | Repo em `F:\repos\ai-memory-manager`, git local, commit por fase |
| Q2 | Fases 1–4 completas; da fase 5 só "Abrir /web" |
| Q3 | Instalador NSIS per-user (sem admin) + `.exe` portátil |
| Q4 | Sem assinatura de código; README explica SmartScreen |
| Q5 | Ferramentas faltantes via winget (só rustup era necessário) |
| Q6 | Ambiente real: só leitura/dry-run. Mutação apenas em sandbox (HOME/USERPROFILE/CODEX_HOME temporários) |
| Q7 | Token bearer só na memória do backend Rust; nunca no frontend/log/arquivo |
| Q8 | Teste MCP em dois níveis: servidor responde (GUI) vs agente carregou (manual/nova sessão) |
| Q9 | Modo demo por fixtures apenas em dev (screenshots), fora do release |
| Q10 | Projeto = pasta escolhida; identidade via `doctor --json`; aviso se não-git com sub-repos |
| Q11 | GUI grava `.ai-memory.toml` com campos documentados na doc local da versão; sem sobrescrever; backup |
| Q12 | Drenar spool com `recover` (detectado via `--help`) |
| Q13 | Desativar agente = edição cirúrgica só das entradas ai-memory, com diff/backup/rollback; `uninstall` só em Avançado |
| Q14 | Controle de captura por projeto: "Não disponível com a integração global atual" |
| Q15 | Servidor: parar via `service-host.stop`; iniciar com o comando do Run; autostart = toggle do valor Run (com backup) |
| Q16 | Codex MCP auth: diagnosticar; corrigir via `install-mcp --client codex` com prévia; env var só com confirmação explícita |
| Q17 | Cursor: MCP suportado; hooks "Experimental — requer Git Bash" |
| Q18 | Instalação: zip + `.sha256` da release oficial, verificação, extração, `setup --plan --json` → `setup --yes --agent ...`; PATH opcional; sem `install.ps1` |
| Q19 | Upgrade: backup das configs → `ai-memory upgrade` → oferecer `install-hooks --apply` |
| Q20 | CLI sempre com `--json`, senão `--language en`; UI em pt-BR |
| Q21 | Backfill: `--dry-run --json` escopado; se no-op, oferecer `--force` (dry-run antes); `--max-sessions` ajustável; cancelável |
| Q22 | Trust Codex: ler `[hooks.state]`/`trusted_hash`; ausente = ação manual no Codex Desktop |
| Q23 | Fixtures a partir de saídas reais sanitizadas (genericizadas: projeto-a/projeto-b) |
| Q24 | Auto-update do app via `tauri-plugin-updater` embutido (NSIS); portátil só avisa |
| Q25 | Releases em `github.com/lucasmonteir0/ai-memory-manager` (público) |
| Q26 | Verificar ao abrir + botão; instalar só com clique; separado do upgrade do ai-memory |
| Q27 | Chave minisign gerada com `tauri signer generate`, privada fora do repo |
| Q28 | GitHub Actions em tag `v*` (principal) + script manual (plano B); push/release só com aprovação |
