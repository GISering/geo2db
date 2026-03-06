use crate::models::DbType;

/// SQL 方言抽象 - 处理不同数据库的 SQL 语法差异
pub trait SqlDialect: Send + Sync {
    /// 获取数据库类型
    fn db_type(&self) -> DbType;

    /// 空间几何类型名称
    fn geometry_type_name(&self) -> &str;

    /// 主键自增定义
    fn auto_increment_pk(&self) -> &str;

    /// WKT 转几何函数
    fn geom_from_wkt(&self, wkt_param: &str, srid_param: &str) -> String;

    /// 映射 GDAL 字段类型到数据库类型
    fn map_field_type(&self, gdal_type: &str) -> &str;

    /// 引用标识符（表名、字段名）
    fn quote_identifier(&self, name: &str) -> String;

    /// 参数占位符
    fn param_placeholder(&self, idx: usize) -> String;

    /// 创建表的 SQL 前缀
    fn create_table_prefix(&self) -> &str;

    /// 检查表是否存在的 SQL
    fn table_exists_sql(&self) -> &str;

    /// 获取表字段的 SQL
    fn get_table_columns_sql(&self) -> &str;

    /// 获取数据库版本的 SQL
    fn version_sql(&self) -> &str;

    /// 生成 CREATE TABLE 语句
    fn create_table_sql(&self, table_name: &str, field_names: &[String], field_types: &[String]) -> String {
        let quoted_table = self.quote_identifier(table_name);
        let mut sql = format!("{} {} (gid {}, geom {}",
            self.create_table_prefix(),
            quoted_table,
            self.auto_increment_pk(),
            self.geometry_type_name()
        );

        for (i, field_name) in field_names.iter().enumerate() {
            let quoted_field = self.quote_identifier(field_name);
            let field_type = self.map_field_type(&field_types[i]);
            sql.push_str(&format!(", {} {}", quoted_field, field_type));
        }
        sql.push(')');
        sql
    }

    /// 生成批量 INSERT 语句（带几何字段）
    fn batch_insert_sql(&self, table_name: &str, field_names: &[String], batch_size: usize) -> String {
        let quoted_table = self.quote_identifier(table_name);
        let quoted_fields: Vec<String> = field_names.iter()
            .map(|f| self.quote_identifier(f))
            .collect();
        let field_count = field_names.len();

        let mut values_parts = Vec::with_capacity(batch_size);
        let mut param_idx = 1;

        for _ in 0..batch_size {
            let geom_expr = self.geom_from_wkt(
                &self.param_placeholder(param_idx),
                &self.param_placeholder(param_idx + 1)
            );
            param_idx += 2;

            let mut row_parts = vec![geom_expr];
            for _ in 0..field_count {
                row_parts.push(self.param_placeholder(param_idx));
                param_idx += 1;
            }
            values_parts.push(format!("({})", row_parts.join(", ")));
        }

        format!(
            "INSERT INTO {} (geom, {}) VALUES {}",
            quoted_table,
            quoted_fields.join(", "),
            values_parts.join(", ")
        )
    }

    /// 生成 DROP TABLE 语句
    fn drop_table_sql(&self, table_name: &str) -> String {
        format!("DROP TABLE IF EXISTS {}", self.quote_identifier(table_name))
    }
}

/// 创建 SQL 方言实例
pub fn create_dialect(db_type: &DbType) -> Box<dyn SqlDialect> {
    match db_type {
        DbType::PostgreSQL => Box::new(super::dialects::postgres::PostgresDialect),
        DbType::Dameng => Box::new(super::dialects::dameng::DamengDialect),
    }
}