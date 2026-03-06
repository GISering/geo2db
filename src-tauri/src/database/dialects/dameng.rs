use crate::database::traits::SqlDialect;
use crate::models::DbType;

/// 达梦数据库 (sysgeo2 空间扩展) SQL 方言实现
pub struct DamengDialect;

impl SqlDialect for DamengDialect {
    fn db_type(&self) -> DbType {
        DbType::Dameng
    }

    fn geometry_type_name(&self) -> &str {
        // 达梦空间扩展需要指定 sysgeo2 schema
        "sysgeo2.ST_GEOMETRY"
    }

    fn auto_increment_pk(&self) -> &str {
        "INT IDENTITY(1,1) PRIMARY KEY"
    }

    fn geom_from_wkt(&self, wkt_param: &str, srid_param: &str) -> String {
        // dmgeo2 空间扩展用于函数调用
        format!("dmgeo2.ST_GeomFromText({}, {})", wkt_param, srid_param)
    }

    fn map_field_type(&self, gdal_type: &str) -> &str {
        match gdal_type.to_uppercase().as_str() {
            "INTEGER" | "INT4" => "INTEGER",
            "INTEGER64" | "INT8" => "BIGINT",
            "REAL" | "FLOAT4" => "FLOAT",
            "DOUBLE" | "FLOAT8" => "DOUBLE",
            "STRING" | "CHAR" | "VARCHAR" => "VARCHAR(255)",
            "DATE" => "DATE",
            "TIME" => "TIME",
            "DATETIME" | "TIMESTAMP" => "DATETIME",
            _ => "TEXT",
        }
    }

    fn quote_identifier(&self, name: &str) -> String {
        // 达梦使用双引号引用标识符
        format!("\"{}\"", name.replace('"', "\"\""))
    }

    fn param_placeholder(&self, _idx: usize) -> String {
        // 达梦使用 ? 作为参数占位符
        "?".to_string()
    }

    fn create_table_prefix(&self) -> &str {
        "CREATE TABLE IF NOT EXISTS"
    }

    fn table_exists_sql(&self) -> &str {
        // 达梦使用 USER_TABLES 视图检查表是否存在
        "SELECT COUNT(*) FROM USER_TABLES WHERE TABLE_NAME = ?"
    }

    fn get_table_columns_sql(&self) -> &str {
        // 达梦使用 USER_TAB_COLUMNS 视图获取表字段
        "SELECT COLUMN_NAME FROM USER_TAB_COLUMNS WHERE TABLE_NAME = ?"
    }

    fn version_sql(&self) -> &str {
        // 达梦版本查询
        "SELECT * FROM V$VERSION"
    }
}