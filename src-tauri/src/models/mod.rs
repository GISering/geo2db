use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 数据库类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DbType {
    #[serde(rename = "PostgreSQL")]
    PostgreSQL,
    #[serde(rename = "Dameng")]
    Dameng,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    pub db_type: DbType,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

/// 带名称的数据库配置（用于保存多个配置）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedDbConfig {
    pub name: String,
    #[serde(flatten)]
    pub config: DbConfig,
}

/// 数据库配置列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfigList {
    pub configs: Vec<NamedDbConfig>,
    pub active_config: Option<String>,  // 当前选中的配置名称
}

/// 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub format: String,
    pub layer_name: String,
    pub feature_count: i64,
    pub geometry_type: String,
    pub fields: Vec<FieldInfo>,
    pub srs: Option<SpatialRefInfo>,
}

/// 字段信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: String,
    pub field_type: String,
}

/// 空间参考信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialRefInfo {
    pub epsg: i32,
    pub proj4: Option<String>,
    pub wkt: Option<String>,
}

/// 导入模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImportMode {
    CreateNew,
    Append,
    Replace,
}

/// 导入配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfig {
    pub db_config: DbConfig,
    pub file_path: String,
    pub layer_name: Option<String>,
    pub table_name: String,
    pub srs: Option<String>,
    pub import_mode: ImportMode,
    pub field_mapping: Option<HashMap<String, String>>,
}

/// 导入进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub current: i64,
    pub total: i64,
    pub status: String,
    pub message: String,
}

/// 导入结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub success: bool,
    pub imported_count: i64,
    pub error_count: i64,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// 连接测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: String,
    pub server_version: Option<String>,
}