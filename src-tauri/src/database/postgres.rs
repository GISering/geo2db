use crate::models::{ConnectionTestResult, DbConfig, DbConfigList, NamedDbConfig};
use postgres::Client;
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "db_config.json";

fn get_config_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or("无法获取配置目录")?
        .join("spatial-import-tool");

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

    log::info!("测试数据库连接: host={}, port={}, dbname={}, user={}",
        config.host, config.port, config.database, config.username);

    match Client::connect(&conn_string, postgres::NoTls) {
        Ok(mut client) => {
            let version: String = client
                .query_one("SELECT version()", &[])
                .map(|row| row.get(0))
                .unwrap_or_else(|_| "Unknown".to_string());

            log::info!("数据库连接成功, 版本: {}", version);
            ConnectionTestResult {
                success: true,
                message: "连接成功".to_string(),
                server_version: Some(version),
            }
        }
        Err(e) => {
            log::error!("数据库连接失败: {}", e);
            ConnectionTestResult {
                success: false,
                message: e.to_string(),
                server_version: None,
            }
        }
    }
}

/// 保存配置（带名称）
pub fn save_config(config: &DbConfig, name: &str) -> Result<(), String> {
    log::info!("开始保存配置: name={}", name);

    let config_dir = dirs::config_dir()
        .ok_or_else(|| "无法获取配置目录".to_string())?
        .join("spatial-import-tool");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("创建配置目录失败: {}", e))?;
    }

    let mut config_list = load_config_list()?;

    if let Some(existing) = config_list.configs.iter_mut().find(|c| c.name == name) {
        existing.config = config.clone();
    } else {
        config_list.configs.push(NamedDbConfig {
            name: name.to_string(),
            config: config.clone(),
        });
    }

    config_list.active_config = Some(name.to_string());
    save_config_list(&config_list)?;

    Ok(())
}

/// 删除配置
pub fn delete_config(name: &str) -> Result<(), String> {
    let mut config_list = load_config_list()?;
    config_list.configs.retain(|c| c.name != name);

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

// ============================================================================
// PostgreSQL 连接和批量插入
// ============================================================================

use postgres::types::ToSql;

/// 创建 PostgreSQL 客户端连接
pub fn create_client(config: &DbConfig) -> Result<Client, String> {
    let conn_string = format!(
        "host={} port={} dbname={} user={} password={}",
        config.host, config.port, config.database, config.username, config.password
    );

    Client::connect(&conn_string, postgres::NoTls)
        .map_err(|e| format!("连接数据库失败: {}", e))
}

/// 字段值枚举 - 支持不同数据类型
#[derive(Clone, Debug)]
pub enum FieldValue {
    Integer(i32),
    Integer64(i64),
    Real(f32),
    Text(String),
    Null,
}

impl ToSql for FieldValue {
    fn to_sql(
        &self,
        ty: &postgres::types::Type,
        out: &mut bytes::BytesMut,
    ) -> Result<postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            FieldValue::Integer(v) => <i32 as ToSql>::to_sql(v, ty, out),
            FieldValue::Integer64(v) => <i64 as ToSql>::to_sql(v, ty, out),
            FieldValue::Real(v) => <f32 as ToSql>::to_sql(v, ty, out),
            FieldValue::Text(v) => <String as ToSql>::to_sql(v, ty, out),
            FieldValue::Null => Ok(postgres::types::IsNull::Yes),
        }
    }

    fn accepts(_ty: &postgres::types::Type) -> bool {
        true
    }

    postgres::types::to_sql_checked!();
}

/// 批量插入数据到 PostgreSQL
pub fn batch_insert_postgres(
    tx: &mut postgres::Transaction,
    table_name: &str,
    field_names: &[String],
    srs: i32,
    batch: &[(String, Vec<FieldValue>)],
) -> Result<usize, String> {
    if batch.is_empty() {
        return Ok(0);
    }

    // 过滤空几何
    let valid_batch: Vec<_> = batch.iter()
        .filter(|(wkt, _)| !wkt.trim().is_empty())
        .collect();

    if valid_batch.is_empty() {
        return Ok(0);
    }

    let batch_size = valid_batch.len();
    let field_count = field_names.len();
    let params_per_row = 2 + field_count; // WKT + SRID + fields

    // 构建 SQL 语句
    let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));
    let quoted_fields: Vec<String> = field_names.iter()
        .map(|f| format!("\"{}\"", f.replace('"', "\"\"")))
        .collect();

    // 动态生成占位符
    let mut values_parts = Vec::with_capacity(batch_size);
    let mut param_idx = 1;

    for _ in 0..batch_size {
        let mut row_parts = vec![
            format!("ST_GeomFromText(${}, ${})", param_idx, param_idx + 1)
        ];
        param_idx += 2;

        for _ in 0..field_count {
            row_parts.push(format!("${}", param_idx));
            param_idx += 1;
        }
        values_parts.push(format!("({})", row_parts.join(", ")));
    }

    let insert_sql = format!(
        "INSERT INTO {} (geom, {}) VALUES {}",
        quoted_table,
        quoted_fields.join(", "),
        values_parts.join(", ")
    );

    log::info!("SQL: {}", insert_sql.chars().take(300).collect::<String>());

    // 准备语句
    let stmt = tx.prepare(&insert_sql)
        .map_err(|e| format!("准备语句失败: {}", e))?;

    // 构建参数数组
    let mut params: Vec<Box<dyn ToSql + Sync>> = Vec::with_capacity(batch_size * params_per_row);

    for (wkt, row_data) in &valid_batch {
        params.push(Box::new(wkt.clone()));
        params.push(Box::new(srs));
        for value in row_data {
            params.push(Box::new(value.clone()));
        }
    }

    let params_refs: Vec<&(dyn ToSql + Sync)> = params.iter()
        .map(|p| p.as_ref())
        .collect();

    let rows_affected = tx.execute(&stmt, &params_refs[..])
        .map_err(|e| format!("INSERT失败: {}", e))?;

    Ok(rows_affected as usize)
}