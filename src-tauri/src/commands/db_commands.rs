use crate::database;
use crate::models::{ConnectionTestResult, DbConfig, DbConfigList};

#[tauri::command]
pub fn test_connection(config: DbConfig) -> ConnectionTestResult {
    database::postgres::test_connection(&config)
}

#[tauri::command]
pub fn save_config(config: DbConfig, name: String) -> Result<(), String> {
    log::info!("收到保存配置请求: name={}, config={:?}", name, config);
    let result = database::postgres::save_config(&config, &name);
    match result {
        Ok(_) => {
            log::info!("配置保存成功");
            Ok(())
        }
        Err(e) => {
            log::error!("保存配置失败: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub fn delete_config(name: String) -> Result<(), String> {
    database::postgres::delete_config(&name)
}

#[tauri::command]
pub fn load_config() -> Result<DbConfigList, String> {
    database::postgres::load_config()
}

#[tauri::command]
pub fn load_active_config() -> Result<Option<DbConfig>, String> {
    database::postgres::load_active_config()
}