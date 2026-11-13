# Testes

## Automatizados

```bash
cargo test --lib --manifest-path src-tauri/Cargo.toml     # 63 testes do backend
npm test                                                  # testes do frontend (vitest)
cargo test --manifest-path src-tauri/Cargo.toml --test real_readonly -- --ignored --nocapture   # opcional: instalação real, somente leitura
```

Cobertura em relação à seção 9 da especificação:

| # | Requisito | Onde |
|---|---|---|
| 1 | Locator: padrão/PATH/manual, rejeita executável inválido | `aimemory::tests::version_requires_program_name`, `runner::tests::missing_exe_is_error_not_shell_fallback`, `commands::detect` |
| 2 | CommandRunner: argv/cwd separados, espaços/acentos, timeout, sem shell, cancelamento | `runner::tests::*` |
| 3 | DoctorParser: projetos, `untrusted`, locais × capturados, spool, marker, saída desconhecida | `aimemory::tests::doctor_*` (fixtures reais sanitizadas) |
| 4 | StatusParser: pages/sessions/observations, auth, FTS, embeddings desabilitados, lixo → "não interpretado" | `aimemory::tests::status_*` |
| 5 | ProjectManager: não sobrescreve marker; Git root; pasta sem Git com sub-repos | `projects::tests::*` |
| 6 | Codex: plugin existente bloqueia hooks standalone duplicados | `agents::tests::codex_plugin_blocks_standalone_install` |
| 7 | Claude Desktop: Code (hooks) separado de Chat (só MCP); MSIX ambíguo não é escolhido às cegas | `agents::tests::claude_code_and_desktop_are_separate`, `claude_desktop_ambiguous_msix_is_not_guessed` |
| 8 | Merge preservando MCP/hooks alheios (Cursor, Claude, Codex TOML) | `configedit::tests::*` |
| 9 | Backfill: simulação obrigatória antes da importação, cancelamento | `commands::backfill_run` exige dry-run com os mesmos parâmetros (≤30 min); cancelamento via `runner::tests::can_cancel` |
| 10 | Disable não faz full uninstall nem apaga dados; desabilitar um agente não toca outro | `changes::tests::disable_hooks_only_touches_target…`, `projects::tests::catalog_add_remove_does_not_touch_folder` |
| 11 | Rollback após alteração parcial e após verificação falha com exit 0 | `changes::tests::rollback_after_partial_failure…`, `verify_failure_rolls_back_even_with_exit_zero`, `backup::tests::*` |
| 12 | Sanitização: tokens fora de UI, logs, backups e relatórios | `sanitize::tests::*`, `audit::tests::*`, `service::tests::config_and_token_from_data_dir` |
| RQ-15 | Corrida, instalação existente, falta de rede, config inválida, app ausente | lock único de mutação (`AppState::lock_mutation`); `aborts_if_file_changed_since_preview`; `invalid_json_blocks_changes`, `invalid_toml_is_error`; `health_unreachable_is_not_ok`; `cursor_hooks_blocked_without_git_bash…` |
| — | Instalador: URLs só do repo oficial, formatos .sha256, zip com path traversal/sem exe | `installer::tests::*` |
| — | UI: alertas (servidor parado, não interpretado, untrusted, spool) e rótulos | `src/lib/ui.test.tsx` |

## Testes manuais (Windows)

O resultado de cada item está em **"Checklist do que foi efetivamente testado"** no relatório final da entrega.
