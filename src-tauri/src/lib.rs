mod commands;
mod database;
mod gdal;
mod models;

use commands::{db_commands, file_commands, import_commands};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志，默认级别为 info
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    // 设置 GDAL 环境变量以减少错误输出 (必须在 GDAL 初始化之前设置)
    std::env::set_var("CPL_LOG", "/dev/null");
    std::env::set_var("CPL_LOG_IS", "NULL");
    std::env::set_var("GDAL_ERROR_HANDLING", "DISABLE_ERROR_PRINTER");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            file_commands::list_files,
            file_commands::get_file_info,
            file_commands::list_layers,
            file_commands::get_supported_drivers,
            db_commands::test_connection,
            db_commands::check_dameng_driver,
            db_commands::save_config,
            db_commands::delete_config,
            db_commands::load_config,
            db_commands::load_active_config,
            import_commands::import_file,
            import_commands::cancel_import,
            import_commands::get_import_progress,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}