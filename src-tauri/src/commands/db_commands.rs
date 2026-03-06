use crate::models::{ConnectionTestResult, DbConfig, DbConfigList, DbType};

#[tauri::command]
pub fn test_connection(config: DbConfig) -> ConnectionTestResult {
    match config.db_type {
        DbType::PostgreSQL => crate::database::postgres::test_connection(&config),
        DbType::Dameng => crate::database::dameng::test_connection(&config),
    }
}

#[tauri::command]
pub fn save_config(config: DbConfig, name: String) -> Result<(), String> {
    log::info!("收到保存配置请求: name={}, config={:?}", name, config);
    // 配置保存对所有数据库类型都是相同的，使用 PostgreSQL 模块的实现
    let result = crate::database::postgres::save_config(&config, &name);
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
    // 配置删除对所有数据库类型都是相同的
    crate::database::postgres::delete_config(&name)
}

#[tauri::command]
pub fn load_config() -> Result<DbConfigList, String> {
    crate::database::postgres::load_config()
}

#[tauri::command]
pub fn load_active_config() -> Result<Option<DbConfig>, String> {
    crate::database::postgres::load_active_config()
}