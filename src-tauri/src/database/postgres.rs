use crate::models::{ConnectionTestResult, DbConfig, DbConfigList, NamedDbConfig};
use postgres::Client;
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "db_config.json";

fn get_config_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or("无法获取配置目录")?
        .join("spatial-import-tool");

    // 确保目录存在
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("创建配置目录失败: {}", e))?;
    }

    Ok(config_dir.join(CONFIG_FILE_NAME))
}

fn load_config_list() -> Result<DbConfigList, String> {
    let config_path = get_config_path()?;

    log::info!("加载配置文件: {:?}", config_path);

    if !config_path.exists() {
        log::info!("配置文件不存在，创建新的");
        let new_config = DbConfigList {
            configs: vec![],
            active_config: None,
        };
        save_config_list(&new_config)?;
        return Ok(new_config);
    }

    let json = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) => {
            log::warn!("读取配置文件失败: {}，创建新的", e);
            let new_config = DbConfigList {
                configs: vec![],
                active_config: None,
            };
            save_config_list(&new_config)?;
            return Ok(new_config);
        }
    };

    log::info!("配置文件内容: {}", json);

    // 尝试解析，如果失败则创建新的
    match serde_json::from_str::<DbConfigList>(&json) {
        Ok(config_list) => Ok(config_list),
        Err(e) => {
            log::warn!("解析配置文件失败: {}，创建新的", e);
            let new_config = DbConfigList {
                configs: vec![],
                active_config: None,
            };
            save_config_list(&new_config)?;
            return Ok(new_config);
        }
    }
}

fn save_config_list(config_list: &DbConfigList) -> Result<(), String> {
    let config_path = get_config_path()?;

    let json = serde_json::to_string_pretty(config_list)
        .map_err(|e| format!("序列化配置失败: {}", e))?;

    fs::write(&config_path, json)
        .map_err(|e| format!("写入配置文件失败: {}", e))?;

    log::info!("数据库配置已保存到: {:?}", config_path);
    Ok(())
}

pub fn test_connection(config: &DbConfig) -> ConnectionTestResult {
    let conn_string = format!(
        "host={} port={} dbname={} user={} password={}",
        config.host, config.port, config.database, config.username, config.password
    );

    match Client::connect(&conn_string, postgres::NoTls) {
        Ok(mut client) => {
            let version: String = client
                .query_one("SELECT version()", &[])
                .map(|row| row.get(0))
                .unwrap_or_else(|_| "Unknown".to_string());

            ConnectionTestResult {
                success: true,
                message: "连接成功".to_string(),
                server_version: Some(version),
            }
        }
        Err(e) => ConnectionTestResult {
            success: false,
            message: e.to_string(),
            server_version: None,
        },
    }
}

/// 保存配置（带名称）
pub fn save_config(config: &DbConfig, name: &str) -> Result<(), String> {
    log::info!("开始保存配置: name={}", name);
    log::info!("配置详情: db_type={:?}, host={}, port={}, database={}, username={}",
        config.db_type, config.host, config.port, config.database, config.username);

    // 检查配置目录
    let config_dir = dirs::config_dir()
        .ok_or_else(|| {
            log::error!("无法获取配置目录");
            "无法获取配置目录".to_string()
        })?
        .join("spatial-import-tool");

    log::info!("配置目录: {:?}", config_dir);

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| {
                log::error!("创建配置目录失败: {}", e);
                format!("创建配置目录失败: {}", e)
            })?;
        log::info!("配置目录已创建");
    }

    let mut config_list = load_config_list()?;

    // 查找并更新或添加配置
    if let Some(existing) = config_list.configs.iter_mut().find(|c| c.name == name) {
        log::info!("更新已有配置: {}", name);
        existing.config = config.clone();
    } else {
        log::info!("添加新配置: {}", name);
        config_list.configs.push(NamedDbConfig {
            name: name.to_string(),
            config: config.clone(),
        });
    }

    // 设置为活动配置
    config_list.active_config = Some(name.to_string());

    save_config_list(&config_list)?;
    log::info!("配置保存完成");

    Ok(())
}

/// 删除配置
pub fn delete_config(name: &str) -> Result<(), String> {
    let mut config_list = load_config_list()?;

    config_list.configs.retain(|c| c.name != name);

    // 如果删除的是活动配置，清除活动配置
    if config_list.active_config.as_deref() == Some(name) {
        config_list.active_config = config_list.configs.first().map(|c| c.name.clone());
    }

    save_config_list(&config_list)
}

/// 加载配置列表
pub fn load_config() -> Result<DbConfigList, String> {
    let config_list = load_config_list()?;
    log::info!("已加载 {} 个数据库配置", config_list.configs.len());
    Ok(config_list)
}

/// 获取活动配置
pub fn load_active_config() -> Result<Option<DbConfig>, String> {
    let config_list = load_config_list()?;

    if let Some(active_name) = &config_list.active_config {
        if let Some(named_config) = config_list.configs.iter().find(|c| &c.name == active_name) {
            return Ok(Some(named_config.config.clone()));
        }
    }

    Ok(None)
}