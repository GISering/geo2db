use crate::database::traits::SqlDialect;
use crate::models::DbType;

/// PostgreSQL/PostGIS SQL 方言实现
pub struct PostgresDialect;

impl SqlDialect for PostgresDialect {
    fn db_type(&self) -> DbType {
        DbType::PostgreSQL
    }

    fn geometry_type_name(&self) -> &str {
        "GEOMETRY"
    }

    fn auto_increment_pk(&self) -> &str {
        "SERIAL PRIMARY KEY"
    }

    fn geom_from_wkt(&self, wkt_param: &str, srid_param: &str) -> String {
        format!("ST_GeomFromText({}, {})", wkt_param, srid_param)
    }

    fn map_field_type(&self, gdal_type: &str) -> &str {
        match gdal_type.to_uppercase().as_str() {
            "INTEGER" | "INT4" => "INTEGER",
            "INTEGER64" | "INT8" => "BIGINT",
            "REAL" | "FLOAT4" => "REAL",
            "DOUBLE" | "FLOAT8" => "DOUBLE PRECISION",
            "STRING" | "CHAR" | "VARCHAR" => "VARCHAR(255)",
            "DATE" => "DATE",
            "TIME" => "TIME",
            "DATETIME" | "TIMESTAMP" => "TIMESTAMP",
            _ => "TEXT",
        }
    }

    fn quote_identifier(&self, name: &str) -> String {
        format!("\"{}\"", name.replace('"', "\"\""))
    }

    fn param_placeholder(&self, idx: usize) -> String {
        format!("${}", idx)
    }

    fn create_table_prefix(&self) -> &str {
        "CREATE TABLE IF NOT EXISTS"
    }

    fn table_exists_sql(&self) -> &str {
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1"
    }

    fn get_table_columns_sql(&self) -> &str {
        "SELECT column_name FROM information_schema.columns WHERE table_name = $1"
    }

    fn version_sql(&self) -> &str {
        "SELECT version()"
    }
}