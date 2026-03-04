use crate::models::{ImportConfig, ImportMode, ImportProgress, ImportResult};
use gdal::vector::LayerAccess;
use gdal::Dataset;
use postgres::{NoTls};
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use log::{info, error};
use postgres::types::ToSql;

// 全局取消标志
static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

/// 字段值枚举 - 支持不同数据类型
#[derive(Clone, Debug)]
enum FieldValue {
    Integer(i32),
    Integer64(i64),
    Real(f32),      // 使用 f32 匹配 PostgreSQL REAL 类型
    Double(f64),    // 用于 DOUBLE PRECISION 类型
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
            FieldValue::Integer(v) => {
                <i32 as ToSql>::to_sql(v, ty, out)
            }
            FieldValue::Integer64(v) => {
                <i64 as ToSql>::to_sql(v, ty, out)
            }
            FieldValue::Real(v) => {
                // f32 对应 PostgreSQL REAL (float4)
                <f32 as ToSql>::to_sql(v, ty, out)
            }
            FieldValue::Double(v) => {
                // f64 对应 PostgreSQL DOUBLE PRECISION (float8)
                <f64 as ToSql>::to_sql(v, ty, out)
            }
            FieldValue::Text(v) => {
                <String as ToSql>::to_sql(v, ty, out)
            }
            FieldValue::Null => Ok(postgres::types::IsNull::Yes),
        }
    }

    fn accepts(_ty: &postgres::types::Type) -> bool {
        true
    }

    postgres::types::to_sql_checked!();
}

#[tauri::command]
pub fn cancel_import() -> bool {
    info!("取消导入命令被调用");
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    true
}

fn get_postgis_type(gdal_type: &str) -> &str {
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

/// 批量插入数据 - 使用预处理语句
fn batch_insert(
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

    // 构建 SQL 语句 - 表名和字段名需要标识符转义
    let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));
    let quoted_fields: Vec<String> = field_names.iter()
        .map(|f| format!("\"{}\"", f.replace('"', "\"\"")))
        .collect();

    // 动态生成占位符: VALUES (ST_GeomFromText($1, $2), $3, $4, ...), (...)
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

    // 打印调试信息
    info!("SQL: {}", insert_sql.chars().take(300).collect::<String>());
    if let Some((wkt, row_data)) = valid_batch.first() {
        info!("第一行数据: WKT长度={}, 字段值={:?}", wkt.len(), row_data);
    }

    // 准备语句
    let stmt = tx.prepare(&insert_sql)
        .map_err(|e| {
            error!("准备语句失败: {:?}", e);
            format!("准备语句失败: {}", e)
        })?;

    // 构建参数数组
    let mut params: Vec<Box<dyn ToSql + Sync>> = Vec::with_capacity(batch_size * params_per_row);

    for (wkt, row_data) in &valid_batch {
        params.push(Box::new(wkt.clone()));      // WKT
        params.push(Box::new(srs));               // SRID
        for value in row_data {
            params.push(Box::new(value.clone())); // 字段值（保持原始类型）
        }
    }

    // 转换为引用切片
    let params_refs: Vec<&(dyn ToSql + Sync)> = params.iter()
        .map(|p| p.as_ref())
        .collect();

    info!("参数数量: {}", params_refs.len());

    // 执行批量插入
    let rows_affected = tx.execute(&stmt, &params_refs[..])
        .map_err(|e| {
            error!("INSERT失败: {:?}", e);
            format!("INSERT失败: {}", e)
        })?;

    Ok(rows_affected as usize)
}

// 后台执行导入
fn do_import_in_background(
    config: ImportConfig,
    app_handle: AppHandle,
) -> ImportResult {
    let start = Instant::now();

    // 发送进度更新
    let send_progress = |current: i64, total: i64, message: &str| {
        info!("发送进度: {}/{} - {}", current, total, message);
        let _ = app_handle.emit("import-progress", ImportProgress {
            current,
            total,
            status: "importing".to_string(),
            message: message.to_string(),
        });
    };

    // 打开数据文件
    let dataset = match Dataset::open(&config.file_path) {
        Ok(d) => d,
        Err(e) => {
            return ImportResult {
                success: false,
                imported_count: 0,
                error_count: 1,
                errors: vec![format!("打开文件失败: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    // 获取指定的图层
    let mut layer = if let Some(ref layer_name) = config.layer_name {
        match dataset.layer_by_name(layer_name) {
            Ok(l) => l,
            Err(e) => {
                return ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec![format!("找不到图层 {}: {}", layer_name, e)],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }
    } else {
        let mut layers = dataset.layers();
        match layers.next() {
            Some(l) => l,
            None => {
                return ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec!["没有找到图层".to_string()],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }
    };

    send_progress(0, 100, "正在读取数据...");

    let defn = layer.defn();
    let mut field_names: Vec<String> = Vec::new();
    let mut field_types: Vec<String> = Vec::new();

    // 获取字段信息，同时保存字段名到 GDAL 实际索引的映射
    let mut field_index_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for field in defn.fields() {
        let field_name = field.name().to_string();
        // 使用 defn 获取字段的实际索引
        let field_idx_result = defn.field_index(&field_name);
        if let Ok(field_idx) = field_idx_result {
            field_index_map.insert(field_name.clone(), field_idx);
        }
        field_names.push(field.name().to_string());
        let ty = field.field_type();
        let ty_name = match ty {
            gdal::vector::OGRFieldType::OFTString => "String",
            gdal::vector::OGRFieldType::OFTInteger => "Integer",
            gdal::vector::OGRFieldType::OFTInteger64 => "Integer64",
            gdal::vector::OGRFieldType::OFTReal => "Real",
            gdal::vector::OGRFieldType::OFTDate => "Date",
            gdal::vector::OGRFieldType::OFTTime => "Time",
            gdal::vector::OGRFieldType::OFTDateTime => "DateTime",
            _ => "String",
        };
        field_types.push(ty_name.to_string());
    }

    // 连接数据库 - 使用连接池
    // 使用 postgres::Config 构建连接配置
    let mut pg_config = postgres::Config::new();
    pg_config.host(&config.db_config.host);
    pg_config.port(config.db_config.port);
    pg_config.dbname(&config.db_config.database);
    pg_config.user(&config.db_config.username);
    pg_config.password(&config.db_config.password);
    pg_config.keepalives(true);
    pg_config.keepalives_idle(std::time::Duration::from_secs(30));

    // 创建连接池
    let manager = PostgresConnectionManager::new(pg_config, NoTls);
    let pool = match r2d2::Pool::new(manager) {
        Ok(p) => p,
        Err(e) => {
            return ImportResult {
                success: false,
                imported_count: 0,
                error_count: 1,
                errors: vec![format!("创建连接池失败: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    // 获取连接用于初始表操作
    let mut client = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return ImportResult {
                success: false,
                imported_count: 0,
                error_count: 1,
                errors: vec![format!("从连接池获取连接失败: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let table_name = &config.table_name;

    // 根据 import_mode 处理表
    match config.import_mode {
        ImportMode::CreateNew | ImportMode::Replace => {
            if matches!(config.import_mode, ImportMode::Replace) {
                if let Err(e) = client.execute(&format!("DROP TABLE IF EXISTS {}", table_name), &[]) {
                    return ImportResult {
                        success: false,
                        imported_count: 0,
                        error_count: 1,
                        errors: vec![format!("删除表失败: {}", e)],
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            }

            let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));
            let mut create_sql = format!("CREATE TABLE IF NOT EXISTS {} (", quoted_table);
            create_sql.push_str("gid SERIAL PRIMARY KEY, geom GEOMETRY");

            for (i, field_name) in field_names.iter().enumerate() {
                let quoted_field = format!("\"{}\"", field_name.replace('"', "\"\""));
                let field_type = get_postgis_type(&field_types[i]);
                create_sql.push_str(&format!(", {} {}", quoted_field, field_type));
            }
            create_sql.push(')');

            if let Err(e) = client.execute(&create_sql, &[]) {
                return ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec![format!("创建表失败: {}", e)],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }

            // 不在导入前创建索引，导入完成后再创建以提升性能
        }
        ImportMode::Append => {
            // 使用预处理语句查询表是否存在
            let check_sql = "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1";
            let exists: i64 = client.query_one(check_sql, &[&table_name])
                .map(|row| row.get(0))
                .unwrap_or(0);

            if exists == 0 {
                return ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec![format!("表 {} 不存在，请使用 CreateNew 模式创建表", table_name)],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }

            // 使用预处理语句获取现有表的字段
            let fields_sql = "SELECT column_name FROM information_schema.columns WHERE table_name = $1";
            let existing_fields: Vec<String> = client.query(fields_sql, &[&table_name])
                .map(|rows| rows.iter().map(|r| r.get::<_, String>(0)).collect())
                .unwrap_or_default();

            info!("表 {} 现有字段: {:?}", table_name, existing_fields);

            // 检查 geom 字段是否存在
            if !existing_fields.contains(&"geom".to_string()) {
                return ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec![format!("表 {} 没有 geom 字段，无法追加数据", table_name)],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }

            // Append 模式下需要删除表重建，因为字段类型可能不匹配
            // 更好的方案是让用户使用 Replace 模式
            return ImportResult {
                success: false,
                imported_count: 0,
                error_count: 1,
                errors: vec![format!("表 {} 已存在，请使用 Replace 模式删除并重建表", table_name)],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    }

    // 获取 SRS
    let srs = if let Some(ref srs_str) = config.srs {
        srs_str
            .strip_prefix("EPSG:")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(4326)
    } else {
        if let Some(layer_srs) = layer.spatial_ref() {
            layer_srs.auth_code().unwrap_or(4326)
        } else {
            4326
        }
    };

    // 先获取总记录数
    let total_count = layer.feature_count() as i64;

    // 流式处理：优化批次大小策略
    // 大数据集使用更大批次以减少事务开销
    let batch_size = if total_count <= 500 {
        10
    } else if total_count <= 10000 {
        100
    } else {
        300
    };

    if total_count == 0 {
        send_progress(0, 1, "没有数据可导入");
        return ImportResult {
            success: true,
            imported_count: 0,
            error_count: 0,
            errors: vec![],
            duration_ms: start.elapsed().as_millis() as u64,
        };
    }

    send_progress(0, total_count, &format!("共 {} 条记录，开始导入...", total_count));
    info!("总记录数: {}, 批次大小: {}", total_count, batch_size);

    // 获取数据库连接并开始一个长事务
    let mut client = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return ImportResult {
                success: false,
                imported_count: 0,
                error_count: 1,
                errors: vec![format!("从连接池获取连接失败: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let mut tx = match client.transaction() {
        Ok(t) => t,
        Err(e) => {
            return ImportResult {
                success: false,
                imported_count: 0,
                error_count: 1,
                errors: vec![format!("开始事务失败: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    // 注意：使用 INSERT INTO VALUES 批量插入，SRID 通过 ST_GeomFromText 函数传递

    // 单次遍历处理所有要素
    let mut batch: Vec<(String, Vec<FieldValue>)> = Vec::with_capacity(batch_size);
    let mut imported_count: i64 = 0;
    let mut error_count: i64 = 0;
    let mut errors: Vec<String> = Vec::new();

    for feat in layer.features() {
        // 检查取消标志
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            info!("导入已取消");
            let _ = app_handle.emit("import-progress", ImportProgress {
                current: imported_count,
                total: total_count,
                status: "cancelled".to_string(),
                message: "导入已取消".to_string(),
            });
            return ImportResult {
                success: false,
                imported_count,
                error_count,
                errors: vec!["用户取消了导入".to_string()],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // 处理单个要素
        let geometry = match feat.geometry() {
            Some(g) => g,
            None => {
                error_count += 1;
                continue;
            }
        };

        let wkt = match geometry.wkt() {
            Ok(w) => w,
            Err(_) => {
                error_count += 1;
                continue;
            }
        };

        let mut row_data: Vec<FieldValue> = Vec::with_capacity(field_names.len());
        for field_name in &field_names {
            let field_idx = field_index_map.get(field_name).copied();
            let field_value = match field_idx {
                Some(idx) if idx < 1000 => {
                    match feat.field(idx) {
                        Ok(Some(field)) => {
                            match field {
                                gdal::vector::FieldValue::IntegerValue(v) => FieldValue::Integer(v),
                                gdal::vector::FieldValue::Integer64Value(v) => FieldValue::Integer64(v),
                                gdal::vector::FieldValue::RealValue(v) => FieldValue::Real(v as f32),
                                gdal::vector::FieldValue::StringValue(v) => {
                                    // 清理特殊字符
                                    let cleaned: String = v.chars()
                                        .filter(|c| *c != '\0' && *c != '\r')
                                        .map(|c| if c == '\n' { ' ' } else { c })
                                        .collect();
                                    FieldValue::Text(cleaned)
                                }
                                _ => FieldValue::Null,
                            }
                        }
                        _ => FieldValue::Null,
                    }
                }
                _ => FieldValue::Null,
            };
            row_data.push(field_value);
        }

        batch.push((wkt, row_data));

        // 达到批次大小时写入数据库
        if batch.len() >= batch_size {
            match batch_insert(&mut tx, table_name, &field_names, srs, &batch) {
                Ok(count) => {
                    imported_count += count as i64;
                    send_progress(imported_count, total_count, &format!("导入中... {}/{}", imported_count, total_count));
                }
                Err(e) => {
                    error!("批次插入失败: {}", e);
                    error_count += batch.len() as i64;
                    if errors.len() < 10 {
                        errors.push(format!("批次插入失败: {}", e));
                    }
                }
            }
            batch.clear();
        }
    }

    // 处理剩余数据
    if !batch.is_empty() {
        match batch_insert(&mut tx, table_name, &field_names, srs, &batch) {
            Ok(count) => {
                imported_count += count as i64;
            }
            Err(e) => {
                error!("最后批次插入失败: {}", e);
                error_count += batch.len() as i64;
                if errors.len() < 10 {
                    errors.push(format!("最后批次插入失败: {}", e));
                }
            }
        }
    }

    // 提交事务
    match tx.commit() {
        Ok(_) => {
            info!("事务提交成功");
        }
        Err(e) => {
            error!("提交事务失败: {}", e);
            return ImportResult {
                success: false,
                imported_count,
                error_count,
                errors: vec![format!("提交事务失败: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    }

    
    let duration = start.elapsed();
    send_progress(total_count, total_count, &format!("导入完成！共导入 {} 条", imported_count));
    info!("导入完成，总耗时: {}ms", duration.as_millis());

    ImportResult {
        success: error_count == 0,
        imported_count,
        error_count,
        errors,
        duration_ms: duration.as_millis() as u64,
    }
}

#[tauri::command]
pub fn import_file(config: ImportConfig, app_handle: AppHandle) -> Result<ImportResult, String> {
    info!("开始导入文件: {}", config.file_path);

    // 重置取消标志
    CANCEL_FLAG.store(false, Ordering::SeqCst);

    // 发送开始信号
    let _ = app_handle.emit("import-progress", ImportProgress {
        current: 0,
        total: 100,
        status: "starting".to_string(),
        message: "正在启动导入...".to_string(),
    });

    // 在后台线程执行导入，不阻塞命令线程
    // 注意：不使用 .join() 等待，让线程在后台运行
    let handle = app_handle.clone();
    let app_handle_for_complete = app_handle.clone();
    std::thread::spawn(move || {
        info!("后台线程启动");
        let result = do_import_in_background(config, handle);
        info!("导入完成，发送结果事件");
        let _ = app_handle_for_complete.emit("import-complete", result);
    });

    // 立即返回，不等待导入完成
    Ok(ImportResult {
        success: true,
        imported_count: 0,
        error_count: 0,
        errors: vec![],
        duration_ms: 0,
    })
}

#[tauri::command]
pub fn get_import_progress() -> ImportProgress {
    ImportProgress {
        current: 0,
        total: 0,
        status: "idle".to_string(),
        message: "".to_string(),
    }
}