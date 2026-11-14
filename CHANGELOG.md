# Changelog

## 0.1.2 — 13/11/2026

- Claude Code + Cursor: o Cursor importa os hooks do Claude Code ignorando `args` e bloqueava todo prompt (`ERROR_HOOKS_BLOCKED`). A central de agentes agora detecta o conflito, oferece converter os hooks do ai-memory para linha única (com prévia e backup) e já aplica a conversão ao instalar os hooks do Claude Code com o Cursor presente.

## 0.1.1 — 13/11/2026

- Primeira release publicada (a tag v0.1.0 falhou no build por BOM no package.json; sem binários).
- Codex: MCP pela ponte local `ai-memory.exe mcp-bridge`, sem gravar token.
- Backfill: importação parcial exibida como parcial, com explicação dos avisos.

## 0.1.0 — 13/11/2026

Primeira versão.

- Detecção e validação do ai-memory (PATH, caminho padrão, seleção manual), estados distintos (não instalado, sem setup, servidor parado, erro de autenticação, saudável, não interpretado).
- Dashboard com contadores, FTS/embeddings separados, spool e alertas acionáveis.
- Catálogo de projetos (seletor, colar caminho, arrastar), doctor com cwd do projeto, marker `.ai-memory.toml` (sem sobrescrever), backfill com simulação obrigatória, `--force` e cancelamento.
- Central de agentes: Claude Desktop (Code), Codex Desktop, Cursor, Claude Desktop (Chat, só MCP), Cowork (não verificado). MCP e hooks separados; configuração pelo instalador oficial; desativação cirúrgica por agente; detecção de trust do Codex, plugin e duplicidade; correção opcional do token do Codex.
- Teste do servidor MCP (com/sem token) e teste de captura fim-a-fim com sessão nova.
- Servidor: iniciar/parar via service-host, início com Windows (HKCU Run).
- Instalação do ai-memory pela release oficial com SHA-256; setup guiado; upgrade com backup.
- Backups com diff e restauração; log de operações sanitizado; relatório sanitizado.
- Atualização automática do próprio app (tauri-plugin-updater, NSIS por usuário).
