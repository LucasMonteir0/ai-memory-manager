pub mod agents;
pub mod aimemory;
pub mod audit;
pub mod backup;
pub mod changes;
pub mod commands;
pub mod configedit;
pub mod installer;
pub mod paths;
pub mod projects;
pub mod runner;
pub mod sanitize;
pub mod service;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = commands::AppState::new(paths::SysPaths::from_env());
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::env_info,
            commands::detect,
            commands::set_manual_binary,
            commands::list_projects,
            commands::inspect_folder,
            commands::add_project,
            commands::remove_project,
            commands::rename_project,
            commands::run_doctor,
            commands::cached_doctor,
            commands::plan_marker,
            commands::backfill_dry_run,
            commands::backfill_run,
            commands::cancel_operation,
            commands::recover_run,
            commands::agents_view,
            commands::plan_agent_change,
            commands::plan_codex_token_fix,
            commands::plan_codex_bridge,
            commands::get_plan,
            commands::discard_plan,
            commands::apply_plan,
            commands::test_mcp,
            commands::capture_test_start,
            commands::capture_test_check,
            commands::server_stop,
            commands::server_start,
            commands::server_health,
            commands::plan_autostart,
            commands::list_backups,
            commands::plan_restore_backup,
            commands::list_operations,
            commands::operation_detail,
            commands::sanitized_report,
            commands::ai_memory_release,
            commands::install_ai_memory,
            commands::setup_plan,
            commands::setup_apply,
            commands::upgrade_ai_memory,
            commands::plan_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o AI Memory Manager");
}
