use crate::models::{ImportConfig, ImportMode, ImportProgress, ImportResult};
use gdal::vector::LayerAccess;
use gdal::Dataset;
use postgres::{NoTls};
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use log::{info, error};

// 全局取消标志
static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

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

// 读取单批要素数据
fn read_feature_batch(
    layer: &mut gdal::vector::Layer,
    field_names: &[String],
    field_index_map: &std::collections::HashMap<String, usize>,
    offset: usize,
    batch_size: usize,
) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut records: Vec<(String, Vec<String>)> = Vec::new();

    // 跳过前面的记录
    let mut skipped = 0;
    for feat in layer.features() {
        if skipped < offset {
            skipped += 1;
            continue;
        }
        if records.len() >= batch_size {
            break;
        }

        let geometry = match feat.geometry() {
            Some(g) => g,
            None => {
                continue;
            }
        };

        let wkt = match geometry.wkt() {
            Ok(w) => w,
            Err(_) => {
                continue;
            }
        };

        let mut row_data: Vec<String> = Vec::new();

        // 通过字段名查找索引并读取，预先检查索引有效性
        for field_name in field_names {
            let field_idx = field_index_map.get(field_name).copied();
            let field_str = match field_idx {
                Some(idx) if idx < 1000 => {  // 简单范围检查
                    match feat.field(idx) {
                        Ok(Some(field)) => {
                            match field {
                                gdal::vector::FieldValue::IntegerValue(v) => v.to_string(),
                                gdal::vector::FieldValue::Integer64Value(v) => v.to_string(),
                                gdal::vector::FieldValue::RealValue(v) => v.to_string(),
                                gdal::vector::FieldValue::StringValue(v) => v.replace('\'', "''").replace('\n', " ").replace('\r', ""),
                                _ => String::new(),
                            }
                        }
                        // 字段不存在或访问失败时静默返回空字符串
                        _ => String::new(),
                    }
                }
                _ => String::new(),
            };
            row_data.push(field_str);
        }

        records.push((wkt, row_data));
    }

    Ok(records)
}

// 批量插入数据
fn batch_insert(
    tx: &mut postgres::Transaction,
    table_name: &str,
    field_names: &[String],
    _srs: i32,
    batch: &[(String, Vec<String>)],
) -> Result<usize, String> {
    if batch.is_empty() {
        return Ok(0);
    }

    // 用双引号包裹字段名和表名，避免特殊字符问题
    let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));
    let quoted_fields: Vec<String> = field_names.iter()
        .map(|f| format!("\"{}\"", f.replace('"', "\"\"")))
        .collect();

    let copy_sql = format!(
        "COPY {} (geom, {}) FROM STDIN WITH (FORMAT text, DELIMITER '|')",
        quoted_table,
        quoted_fields.join(", ")
    );

    info!("执行COPY: {}", copy_sql);

    let mut writer = tx.copy_in(&copy_sql)
        .map_err(|e| format!("开始COPY失败: {} - SQL: {}", e, copy_sql))?;

    for (wkt, row_data) in batch {
        // 验证WKT不为空
        if wkt.trim().is_empty() {
            continue;
        }
        let mut row_parts: Vec<String> = vec![wkt.clone()];
        row_parts.extend(row_data.iter().cloned());
        let line = row_parts.join("|");
        if let Err(e) = writeln!(writer, "{}", line) {
            return Err(format!("写入数据失败: {}", e));
        }
    }

    // 确保所有数据都已写入
    writer.flush()
        .map_err(|e| format!("刷新缓冲区失败: {}", e))?;

    writer.finish()
        .map_err(|e| format!("完成COPY失败: {}", e))?;

    Ok(batch.len())
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
    let layer = if let Some(ref layer_name) = config.layer_name {
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
    let mut imported_count: i64 = 0;
    let mut error_count: i64 = 0;
    let mut errors: Vec<String> = Vec::new();

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

            let _ = client.execute(
                &format!("CREATE INDEX IF NOT EXISTS {}_geom_idx ON {} USING GIST(geom)", quoted_table, quoted_table),
                &[]
            );
        }
        ImportMode::Append => {
            let check_sql = format!(
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = '{}'",
                table_name
            );
            let exists: i64 = client.query_one(&check_sql, &[])
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

            // 获取现有表的字段
            let fields_sql = format!(
                "SELECT column_name FROM information_schema.columns WHERE table_name = '{}'",
                table_name
            );
            let existing_fields: Vec<String> = client.query(&fields_sql, &[])
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

    // 流式处理：根据数据量动态设置批次大小
    // 确保至少有 10 批，保证进度更新次数
    let batch_size = if total_count <= 100 {
        total_count as usize / 10 + 1
    } else if total_count <= 1000 {
        50
    } else {
        100
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

    // 循环读取并导入，使用连接池获取连接
    let mut offset = 0;
    loop {
        // 检查取消标志
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            info!("导入已取消");
            let _ = app_handle.emit("import-progress", ImportProgress {
                current: offset as i64,
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

        // 从连接池获取连接
        let mut client = match pool.get() {
            Ok(c) => c,
            Err(e) => {
                error!("从连接池获取连接失败: {}", e);
                error_count += 1;
                if errors.len() < 10 {
                    errors.push(format!("从连接池获取连接失败: {}", e));
                }
                break;
            }
        };

        // 开始事务
        let mut tx = match client.transaction() {
            Ok(t) => t,
            Err(e) => {
                error!("开始事务失败: {}", e);
                error_count += 1;
                if errors.len() < 10 {
                    errors.push(format!("开始事务失败: {}", e));
                }
                break;
            }
        };

        // 设置 SRID，检查是否成功
        let srid_result = tx.execute(&format!("SET postgis.gs_srid TO {}", srs), &[]);
        if let Err(e) = srid_result {
            error!("设置SRID失败: {}，继续尝试导入", e);
        }

        // 重新打开数据集获取新的迭代器
        let dataset = match Dataset::open(&config.file_path) {
            Ok(d) => d,
            Err(e) => {
                error!("打开文件失败: {}", e);
                break;
            }
        };

        // 获取指定的图层
        let mut layer = if let Some(ref layer_name) = config.layer_name {
            match dataset.layer_by_name(layer_name) {
                Ok(l) => l,
                Err(e) => {
                    error!("找不到图层 {}: {}", layer_name, e);
                    break;
                }
            }
        } else {
            let mut layers = dataset.layers();
            match layers.next() {
                Some(l) => l,
                None => break,
            }
        };

        let batch = match read_feature_batch(&mut layer, &field_names, &field_index_map, offset, batch_size) {
            Ok(b) => b,
            Err(e) => {
                error!("读取数据失败: {}", e);
                error_count += 1;
                if errors.len() < 10 {
                    errors.push(format!("读取数据失败: {}", e));
                }
                break;
            }
        };

        if batch.is_empty() {
            info!("没有更多数据，退出循环");
            break;
        }

        // 立即写入数据库
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

        offset += batch.len();
        info!("已导入 {} 条", offset);

        // 如果读取的少于batch_size，说明已是最后一批
        if batch.len() < batch_size {
            // 提交事务
            let commit_result = tx.commit();

            match commit_result {
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
            break;
        }
    }

    let duration = start.elapsed();
    send_progress(total_count, total_count, &format!("导入完成！共导入 {} 条", imported_count));

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