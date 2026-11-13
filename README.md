# AI Memory Manager

Aplicativo desktop para Windows (Tauri v2 + React + TypeScript) que gerencia o [ai-memory](https://github.com/akitaonrails/ai-memory) sem usar o terminal. Ele detecta, instala, diagnostica e integra o ai-memory com o **Claude Desktop (aba Code)**, o **Codex Desktop** e o **Cursor**.

- **Não precisa dos CLIs `claude` nem `codex`.** O app trabalha com o `ai-memory.exe` e com os arquivos de configuração dos apps desktop.
- **Preserva instalações existentes.** Na primeira abertura ele só lê: não roda `setup` e não altera nenhum arquivo.
- **Toda alteração passa pela mesma sequência:** prévia (diff) → confirmação → backup → execução → verificação → rollback automático se falhar.

## Instalar

1. Baixe `AI Memory Manager_X.Y.Z_x64-setup.exe` em [Releases](https://github.com/lucasmonteir0/ai-memory-manager/releases).
2. Execute o instalador. Ele instala só para o seu usuário e não pede administrador.
3. O binário não é assinado com certificado, então o Windows SmartScreen pode mostrar "Editor desconhecido". Clique em **Mais informações → Executar assim mesmo**.

Depois de instalado, o app verifica atualizações ao abrir e pelo botão em **Configurações**. A instalação de uma versão nova só acontece quando você clica em **Atualizar e reiniciar**. O pacote é verificado com assinatura minisign.

Há também o executável portátil `src-tauri/target/release/ai-memory-manager.exe`. Ele funciona sem instalar, mas não se atualiza sozinho.

## Uso rápido

| Tela | O que faz |
|---|---|
| **Visão geral** | Mostra o estado do servidor, os contadores (páginas, sessões, observações, spool, FTS, embeddings) e alertas que levam à tela certa. |
| **Projetos** | Adiciona uma pasta (pelo seletor, colando o caminho ou arrastando) e roda o `doctor` **com o cwd do projeto**. Também cria o marker `.ai-memory.toml` (opcional) e faz o backfill (simulação primeiro, com `--force` quando o ai-memory pula o projeto). Remover um projeto tira ele só da lista. |
| **Agentes** | Por agente, mostra o MCP e os hooks separados, a origem e o escopo, e tem os botões Configurar / Reparar / Desconectar / Desativar captura. Inclui o teste do servidor MCP (com e sem token) e o teste de captura fim-a-fim com uma sessão nova. |
| **Memórias** | Abre a UI web nativa (`/web`) e mostra o status dos modelos opcionais (somente leitura). |
| **Atividades** | Log de operações com filtros, drenagem do spool (`ai-memory recover`) e o botão **Copiar relatório sanitizado**. |
| **Configurações** | Binário (com seleção manual), servidor (iniciar/parar), início com o Windows, instalação, setup e atualização do ai-memory, atualização do app, backups (restaurar) e opções avançadas. |

### Limitações honestas

- **Desativar a captura de um único projeto não está disponível.** Os hooks são globais. `[capture] ignore_paths` não é oferecido como "desativar", porque só filtra eventos ligados a caminhos.
- **Hooks do Cursor no Windows nativo são experimentais.** O ai-memory 3.4.6 gera scripts `.sh`, então o botão só aparece se o Git Bash existir.
- **Claude Desktop Chat é só MCP.** Não há captura automática.
- **A autorização (trust) dos hooks do Codex é manual.** O app detecta hooks não autorizados (via `doctor`) e explica o que fazer, mas não grava trust.
- **`ai-memory uninstall` não filtra por agente.** Por isso fica em "Avançado", com aviso. Para desativar um agente só, o app faz a edição cirúrgica do arquivo daquele agente.
- **O `project init` do ai-memory é interativo.** Por isso o app grava o marker ele mesmo, usando só as chaves documentadas (`workspace` e `project`).
- **Nesta versão do ai-memory, o drenador do spool é `recover`.** O comando `hook-drain` não existe na 3.4.6; o app detecta qual dos dois está disponível.

## Desenvolvimento

Pré-requisitos: Node 22+, Rust stable (MSVC), Visual Studio Build Tools com C++ e WebView2.

```bash
npm install
npm run tauri dev      # app completo
npm run dev:demo       # só o frontend, com fixtures (sem tocar no sistema) — para screenshots
npm test               # testes do frontend (vitest)
cargo test --lib --manifest-path src-tauri/Cargo.toml   # testes do backend
npm run tauri build    # instalador NSIS em src-tauri/target/release/bundle/nsis
```

Cenários do modo demonstração: `http://localhost:1420/?cenario=ausente#overview`, `?cenario=parado` ou o padrão (instalado).

## Releases e atualização automática

1. **Gere a chave do updater uma vez.** Guarde backup dela: sem essa chave, as instalações existentes deixam de aceitar atualizações.

   ```bash
   npx tauri signer generate -w "%USERPROFILE%\.tauri\ai-memory-manager.key"
   ```

2. **Confira o `plugins.updater.pubkey` do `src-tauri/tauri.conf.json`**: ele deve ser o conteúdo do `.key.pub`. A chave de produção já está configurada; só troque se gerar uma chave nova.
3. **Cadastre os secrets do GitHub.** No repositório, em *Settings → Secrets → Actions*, crie `TAURI_SIGNING_PRIVATE_KEY` (conteúdo do `.key`) e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
4. **Publique.** Ajuste a versão em `package.json`, `src-tauri/Cargo.toml` e `src-tauri/tauri.conf.json`, depois rode `git tag vX.Y.Z && git push --tags`. O workflow `.github/workflows/release.yml` testa, gera o build, assina e publica o `latest.json`.

O plano B é o script `scripts/release-local.ps1 -Version X.Y.Z [-Publish]`, que faz o build local e publica com o `gh`.

O repositório precisa ser **público**, porque o updater baixa `releases/latest/download/latest.json` sem autenticação.

Veja também [ARCHITECTURE.md](ARCHITECTURE.md), [docs/TESTING.md](docs/TESTING.md) e [docs/DECISIONS.md](docs/DECISIONS.md).
