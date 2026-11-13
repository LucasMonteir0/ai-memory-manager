# Arquitetura

```text
React (renderer, sem acesso ao sistema de arquivos)
  ├─ pages/  Visão geral · Projetos · Agentes · Memórias · Atividades · Configurações
  ├─ components/Planner.tsx  prévia → confirmação → resultado (diff, verificação, rollback)
  └─ lib/api.ts  invoke() tipado; lib/demo.ts só no modo `dev:demo`
             │  comandos Tauri (src-tauri/src/commands.rs)
             ▼
Rust (backend)
  ├─ aimemory.rs   AiMemoryLocator + parsers status/doctor/backfill/setup --json (fallback "Não interpretado")
  ├─ runner.rs     ProcessRunner: argv separado, cwd, timeout, cancelamento, streaming, sem shell
  ├─ service.rs    saúde (/healthz), token só em memória, teste MCP, service-host start/stop, HKCU Run
  ├─ projects.rs   catálogo da GUI, inspeção de pasta (git root, sub-repos), marker .ai-memory.toml
  ├─ agents.rs     detecção de apps (MSIX/instalação), diagnóstico MCP × hooks por agente
  ├─ configedit.rs edição cirúrgica JSON/TOML (só entradas do ai-memory)
  ├─ changes.rs    motor de alterações: plano, backup, execução, verificação, rollback
  ├─ backup.rs     snapshots + diff + restauração, escrita atômica
  ├─ installer.rs  release oficial + SHA-256 + extração transacional
  ├─ audit.rs      log de operações sanitizado
  └─ sanitize.rs   redação de tokens/headers/segredos
             │
             ▼
ai-memory.exe (instalação única) + configurações nativas dos agentes
```

## Princípios

- **Uma instalação global; projetos são só catálogo + identidade.** Registrar um projeto nunca roda `setup` nem reinstala hooks.
- **O ai-memory é a fonte da verdade.** Nada de SQLite direto; tudo passa por `ai-memory.exe … --json`.
- **MCP ≠ hooks ≠ dados armazenados.** Três estados independentes, cada um com sua evidência.
- **Nenhuma alteração sem prévia.** Todo comando mutável vira um `ChangePlan`: o frontend mostra arquivos, diff, comando, avisos; só então `apply_plan` executa com backup, verifica o resultado relendo o arquivo e restaura o backup se a verificação falhar (mesmo com exit code 0).
- **Um mutador por vez.** `AppState.mutation` (try_lock) recusa operações concorrentes.
- **Token bearer.** Lido do diretório de dados apenas pelo backend, registrado no sanitizador e nunca serializado para o frontend, logs, backups ou relatórios.

## Arquivos que a GUI pode alterar (sempre com prévia e backup)

| Ação | Arquivo/chave | Mecanismo |
|---|---|---|
| Configurar/reparar MCP | `~/.claude.json`, `CODEX_HOME/config.toml`, `~/.cursor/mcp.json`, `claude_desktop_config.json` | `ai-memory install-mcp --client X --config-file <arquivo> --apply` |
| Configurar/reparar hooks | `~/.claude/settings.json`, `CODEX_HOME/hooks.json` (+ `config.toml` no backup), `~/.cursor/hooks.json` | `ai-memory install-hooks --agent X --config-file <arquivo> --apply` |
| Desconectar MCP de um agente | o arquivo MCP daquele agente | edição cirúrgica (remove só `ai-memory`) |
| Desativar captura de um agente | o arquivo de hooks daquele agente | edição cirúrgica (remove só hooks que chamam o ai-memory) |
| Marker | `<projeto>/.ai-memory.toml` | criação com `create_new` (nunca sobrescreve), só `workspace`/`project` |
| Início com Windows | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\ai-memory` | remove/recria o valor (valor salvo no backup) |
| Parar servidor | `<data_dir>/service-host.stop` | mecanismo documentado do service-host |
| Token do Codex (opcional) | `HKCU\Environment\AI_MEMORY_AUTH_TOKEN` | só se não existir; valor nunca vai para backup |
| Instalar ai-memory | `%LOCALAPPDATA%\Programs\ai-memory` (+ PATH do usuário opcional) | zip oficial + SHA-256 |
| Desinstalar (avançado) | todos os agentes | `ai-memory uninstall --only X --apply --yes` (sem `--purge-data`) |

Dados da própria GUI: `%APPDATA%\AI Memory Manager\` (`projects.json`, `settings.json`, `backups\`, `logs\`).

## Adaptadores de agentes

Cada agente é uma função `*_report(ctx) -> AgentReport` em `agents.rs` e uma linha em `agent_files()`. Para incluir um novo agente suportado pelo ai-memory: detectar o app, apontar o arquivo de config, e decidir o formato (JSON `mcpServers`/`hooks` já é genérico; TOML segue o padrão do Codex).
