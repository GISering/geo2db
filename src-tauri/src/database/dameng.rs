use crate::models::{ConnectionTestResult, DbConfig};
use std::sync::Arc;

use odbc_api::{Environment, ConnectionOptions, Cursor};

/// 构建 ODBC 连接字符串
fn build_connection_string(config: &DbConfig) -> String {
    format!(
        "Driver={{DM8 ODBC DRIVER}};Server={};Port={};UID={};PWD={}",
        config.host, config.port, config.username, config.password
    )
}

/// 测试达梦数据库连接
pub fn test_connection(config: &DbConfig) -> ConnectionTestResult {
    let conn_str = build_connection_string(config);

    log::info!("测试达梦数据库连接: {}", conn_str.replace(&config.password, "****"));

    let env = match Environment::new() {
        Ok(e) => e,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: format!("创建 ODBC 环境失败: {:?}", e),
                server_version: None,
            };
        }
    };

    let result = match env.connect_with_connection_string(&conn_str, ConnectionOptions::default()) {
        Ok(_conn) => {
            ConnectionTestResult {
                success: true,
                message: "连接成功".to_string(),
                server_version: Some("达梦数据库 (DM8)".to_string()),
            }
        }
        Err(e) => {
            log::error!("达梦数据库连接失败: {:?}", e);
            ConnectionTestResult {
                success: false,
                message: format!("连接失败: {:?}", e),
                server_version: None,
            }
        }
    };

    result
}

// ============================================================================
// 达梦数据库连接和导入实现
// ============================================================================

/// 达梦数据库连接包装
pub struct DamengConnection {
    #[allow(dead_code)]
    env: Arc<Environment>,
    conn: Option<odbc_api::Connection<'static>>,
}

impl Drop for DamengConnection {
    fn drop(&mut self) {
        // 安全地关闭连接，避免 panic
        if let Some(conn) = self.conn.take() {
            // 使用 catch_unwind 防止 drop 时 panic
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // 显式关闭连接
                drop(conn);
            }));
        }
    }
}

impl DamengConnection {
    /// 创建新连接
    pub fn new(config: &DbConfig) -> Result<Self, String> {
        let conn_str = build_connection_string(config);

        let env = Arc::new(
            Environment::new()
                .map_err(|e| format!("创建 ODBC 环境失败: {:?}", e))?
        );

        let env_ptr = Arc::as_ptr(&env) as *const Environment;
        let env_ref = unsafe { &*env_ptr };

        let conn = env_ref
            .connect_with_connection_string(&conn_str, ConnectionOptions::default())
            .map_err(|e| format!("连接达梦数据库失败: {:?}", e))?;

        let conn_static = unsafe {
            std::mem::transmute::<odbc_api::Connection<'_>, odbc_api::Connection<'static>>(conn)
        };

        Ok(DamengConnection {
            env,
            conn: Some(conn_static),
        })
    }

    /// 执行 SQL（不带参数）
    pub fn execute(&self, sql: &str) -> Result<(), String> {
        self.conn.as_ref().unwrap().execute(sql, ())
            .map_err(|e| format!("执行 SQL 失败: {:?}", e))?;
        Ok(())
    }

    /// 检查表是否存在
    pub fn table_exists(&mut self, table_name: &str) -> Result<bool, String> {
        let sql = format!(
            "SELECT COUNT(*) FROM USER_TABLES WHERE TABLE_NAME = '{}'",
            escape_sql_string(&table_name.to_uppercase())
        );

        match self.conn.as_mut().unwrap().execute(&sql, ()) {
            Ok(Some(mut cursor)) => {
                match cursor.next_row() {
                    Ok(Some(mut row)) => {
                        let mut buf = Vec::new();
                        match row.get_text(1, &mut buf) {
                            Ok(has_value) => {
                                if has_value {
                                    let count_str = String::from_utf8_lossy(&buf);
                                    let count: i64 = count_str.trim().parse().unwrap_or(0);
                                    Ok(count > 0)
                                } else {
                                    Ok(false)
                                }
                            }
                            Err(e) => Err(format!("读取计数失败: {:?}", e)),
                        }
                    }
                    Ok(None) => Ok(false),
                    Err(e) => Err(format!("获取行失败: {:?}", e)),
                }
            }
            Ok(None) => Ok(false),
            Err(e) => Err(format!("执行查询失败: {:?}", e)),
        }
    }

    /// 获取表的字段列表
    pub fn get_table_columns(&mut self, table_name: &str) -> Result<Vec<String>, String> {
        let sql = format!(
            "SELECT COLUMN_NAME FROM USER_TAB_COLUMNS WHERE TABLE_NAME = '{}' ORDER BY COLUMN_ID",
            escape_sql_string(&table_name.to_uppercase())
        );

        match self.conn.as_mut().unwrap().execute(&sql, ()) {
            Ok(Some(mut cursor)) => {
                let mut columns = Vec::new();

                loop {
                    match cursor.next_row() {
                        Ok(Some(mut row)) => {
                            let mut buf = Vec::new();
                            match row.get_text(1, &mut buf) {
                                Ok(true) => {
                                    let name = String::from_utf8_lossy(&buf).to_string();
                                    columns.push(name);
                                }
                                Ok(false) => {} // NULL 值，跳过
                                Err(e) => {
                                    log::warn!("读取列名失败: {:?}", e);
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            log::warn!("获取行失败: {:?}", e);
                            break;
                        }
                    }
                }

                Ok(columns)
            }
            Ok(None) => Ok(Vec::new()),
            Err(e) => Err(format!("执行查询失败: {:?}", e)),
        }
    }

    /// 开始事务
    pub fn begin_transaction(&mut self) -> Result<DamengTransaction, String> {
        self.conn.as_mut().unwrap().set_autocommit(false)
            .map_err(|e| format!("关闭自动提交失败: {:?}", e))?;

        Ok(DamengTransaction {
            conn: self,
            committed: false,
        })
    }
}

/// 达梦数据库事务
pub struct DamengTransaction<'a> {
    conn: &'a mut DamengConnection,
    committed: bool,
}

impl<'a> DamengTransaction<'a> {
    /// 批量插入数据
    pub fn batch_insert(
        &mut self,
        table_name: &str,
        field_names: &[String],
        srs: i32,
        batch: &[(String, Vec<DamengFieldValue>)],
    ) -> Result<usize, String> {
        let conn = self.conn.conn.as_mut().unwrap();
        batch_insert_dameng_impl(conn, table_name, field_names, srs, batch)
    }

    /// 提交事务
    pub fn commit(mut self) -> Result<(), String> {
        self.conn.conn.as_mut().unwrap().commit()
            .map_err(|e| format!("提交事务失败: {:?}", e))?;
        self.committed = true;

        self.conn.conn.as_mut().unwrap().set_autocommit(true)
            .map_err(|e| format!("恢复自动提交失败: {:?}", e))?;

        Ok(())
    }
}

impl<'a> Drop for DamengTransaction<'a> {
    fn drop(&mut self) {
        if !self.committed {
            if let Some(conn) = self.conn.conn.as_mut() {
                let _ = conn.rollback();
            }
        }
        if let Some(conn) = self.conn.conn.as_mut() {
            let _ = conn.set_autocommit(true);
        }
    }
}

/// 字段值枚举 - 支持不同数据类型
#[derive(Clone, Debug)]
pub enum DamengFieldValue {
    Integer(i32),
    Integer64(i64),
    Real(f32),
    Double(f64),
    Text(String),
    Null,
}

/// 批量插入数据到达梦数据库
pub fn batch_insert_dameng(
    conn: &mut DamengConnection,
    table_name: &str,
    field_names: &[String],
    srs: i32,
    batch: &[(String, Vec<DamengFieldValue>)],
) -> Result<usize, String> {
    let conn = conn.conn.as_mut().unwrap();
    batch_insert_dameng_impl(conn, table_name, field_names, srs, batch)
}

/// 批量插入实现 - 直接操作 ODBC 连接
fn batch_insert_dameng_impl(
    conn: &mut odbc_api::Connection<'_>,
    table_name: &str,
    field_names: &[String],
    srs: i32,
    batch: &[(String, Vec<DamengFieldValue>)],
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

    // 构建 SQL 语句
    let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));
    let quoted_fields: Vec<String> = field_names.iter()
        .map(|f| format!("\"{}\"", f.replace('"', "\"\"")))
        .collect();

    let mut inserted = 0;

    for (wkt, row_data) in &valid_batch {
        // 构建值列表
        let mut values = vec![
            format!("dmgeo2.ST_GeomFromText('{}', {})", escape_sql_string(wkt), srs)
        ];

        for value in row_data {
            let v = match value {
                DamengFieldValue::Integer(v) => v.to_string(),
                DamengFieldValue::Integer64(v) => v.to_string(),
                DamengFieldValue::Real(v) => v.to_string(),
                DamengFieldValue::Double(v) => v.to_string(),
                DamengFieldValue::Text(v) => format!("'{}'", escape_sql_string(v)),
                DamengFieldValue::Null => "NULL".to_string(),
            };
            values.push(v);
        }

        let insert_sql = format!(
            "INSERT INTO {} (geom, {}) VALUES ({})",
            quoted_table,
            quoted_fields.join(", "),
            values.join(", ")
        );

        match conn.execute(&insert_sql, ()) {
            Ok(_) => inserted += 1,
            Err(e) => {
                log::error!("插入失败: {:?}", e);
            }
        }
    }

    Ok(inserted)
}

/// 转义 SQL 字符串中的特殊字符
fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
        .replace('\\', "\\\\")
}

/// 检查达梦 ODBC 驱动是否已安装
pub fn check_driver_installed() -> bool {
    if let Ok(env) = Environment::new() {
        if let Ok(drivers) = env.drivers() {
            for driver in drivers {
                let desc = &driver.description;
                if desc.contains("DM8") || desc.to_lowercase().contains("dameng") {
                    return true;
                }
            }
        }
    }
    false
}

/// 创建达梦数据库连接
pub fn create_connection(config: &DbConfig) -> Result<DamengConnection, String> {
    DamengConnection::new(config)
}