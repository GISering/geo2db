use crate::models::{ImportConfig, ImportMode, ImportProgress, ImportResult, DbType};
use crate::database::create_dialect;
use crate::database::postgres::{create_client, batch_insert_postgres, FieldValue};
use crate::database::dameng::{create_connection, DamengFieldValue};
use gdal::vector::LayerAccess;
use gdal::Dataset;
use std::sync::atomic::{AtomicBool, Ordering};
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

// 后台执行导入
fn do_import_in_background(
    config: ImportConfig,
    app_handle: AppHandle,
) -> ImportResult {
    let start = Instant::now();

    // 根据数据库类型选择导入方式
    match config.db_config.db_type {
        DbType::PostgreSQL => do_import_postgres(config, app_handle, start),
        DbType::Dameng => do_import_dameng(config, app_handle, start),
    }
}

/// PostgreSQL 导入实现
fn do_import_postgres(
    config: ImportConfig,
    app_handle: AppHandle,
    start: Instant,
) -> ImportResult {
    let dialect = create_dialect(&DbType::PostgreSQL);

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
    let mut field_index_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for field in defn.fields() {
        let field_name = field.name().to_string();
        if let Ok(field_idx) = defn.field_index(&field_name) {
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

    // 创建数据库连接
    let mut client = match create_client(&config.db_config) {
        Ok(c) => c,
        Err(e) => {
            return ImportResult {
                success: false,
                imported_count: 0,
                error_count: 1,
                errors: vec![e],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let table_name = &config.table_name;

    // 根据 import_mode 处理表
    match config.import_mode {
        ImportMode::CreateNew | ImportMode::Replace => {
            if matches!(config.import_mode, ImportMode::Replace) {
                let drop_sql = dialect.drop_table_sql(table_name);
                if let Err(e) = client.execute(&drop_sql, &[]) {
                    return ImportResult {
                        success: false,
                        imported_count: 0,
                        error_count: 1,
                        errors: vec![format!("删除表失败: {}", e)],
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            }

            let create_sql = dialect.create_table_sql(table_name, &field_names, &field_types);
            if let Err(e) = client.execute(&create_sql, &[]) {
                return ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec![format!("创建表失败: {}", e)],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }
        ImportMode::Append => {
            let check_sql = dialect.table_exists_sql();
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

            let fields_sql = dialect.get_table_columns_sql();
            let existing_fields: Vec<String> = client.query(fields_sql, &[&table_name])
                .map(|rows| rows.iter().map(|r| r.get::<_, String>(0)).collect())
                .unwrap_or_default();

            info!("表 {} 现有字段: {:?}", table_name, existing_fields);

            if !existing_fields.contains(&"geom".to_string()) {
                return ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec![format!("表 {} 没有 geom 字段，无法追加数据", table_name)],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }

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

    let total_count = layer.feature_count() as i64;
    let batch_size = if total_count <= 500 { 10 } else if total_count <= 10000 { 100 } else { 300 };

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

    let mut batch: Vec<(String, Vec<FieldValue>)> = Vec::with_capacity(batch_size);
    let mut imported_count: i64 = 0;
    let mut error_count: i64 = 0;
    let mut errors: Vec<String> = Vec::new();

    for feat in layer.features() {
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

        let geometry = match feat.geometry() {
            Some(g) => g,
            None => { error_count += 1; continue; }
        };

        let wkt = match geometry.wkt() {
            Ok(w) => w,
            Err(_) => { error_count += 1; continue; }
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

        if batch.len() >= batch_size {
            match batch_insert_postgres(&mut tx, table_name, &field_names, srs, &batch) {
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

    if !batch.is_empty() {
        match batch_insert_postgres(&mut tx, table_name, &field_names, srs, &batch) {
            Ok(count) => imported_count += count as i64,
            Err(e) => {
                error!("最后批次插入失败: {}", e);
                error_count += batch.len() as i64;
                if errors.len() < 10 {
                    errors.push(format!("最后批次插入失败: {}", e));
                }
            }
        }
    }

    match tx.commit() {
        Ok(_) => info!("事务提交成功"),
        Err(e) => {
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

/// 达梦数据库导入实现
fn do_import_dameng(
    config: ImportConfig,
    app_handle: AppHandle,
    start: Instant,
) -> ImportResult {
    let dialect = create_dialect(&DbType::Dameng);

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
    let mut field_index_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for field in defn.fields() {
        let field_name = field.name().to_string();
        if let Ok(field_idx) = defn.field_index(&field_name) {
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

    // 创建数据库连接
    let mut conn = match create_connection(&config.db_config) {
        Ok(c) => c,
        Err(e) => {
            return ImportResult {
                success: false,
                imported_count: 0,
                error_count: 1,
                errors: vec![format!("连接达梦数据库失败: {}", e)],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let table_name = &config.table_name;

    // 根据 import_mode 处理表
    match config.import_mode {
        ImportMode::CreateNew | ImportMode::Replace => {
            if matches!(config.import_mode, ImportMode::Replace) {
                let drop_sql = dialect.drop_table_sql(table_name);
                if let Err(e) = conn.execute(&drop_sql) {
                    // 表可能不存在，忽略错误
                    info!("删除表时出错（可能表不存在）: {}", e);
                }
            }

            let create_sql = dialect.create_table_sql(table_name, &field_names, &field_types);
            if let Err(e) = conn.execute(&create_sql) {
                return ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec![format!("创建表失败: {}", e)],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }
        ImportMode::Append => {
            match conn.table_exists(table_name) {
                Ok(true) => {
                    // 检查是否有 geom 字段
                    match conn.get_table_columns(table_name) {
                        Ok(columns) => {
                            if !columns.iter().any(|c| c.to_lowercase() == "geom") {
                                return ImportResult {
                                    success: false,
                                    imported_count: 0,
                                    error_count: 1,
                                    errors: vec![format!("表 {} 没有 geom 字段，无法追加数据", table_name)],
                                    duration_ms: start.elapsed().as_millis() as u64,
                                };
                            }
                        }
                        Err(e) => {
                            return ImportResult {
                                success: false,
                                imported_count: 0,
                                error_count: 1,
                                errors: vec![format!("获取表字段失败: {}", e)],
                                duration_ms: start.elapsed().as_millis() as u64,
                            };
                        }
                    }
                }
                Ok(false) => {
                    return ImportResult {
                        success: false,
                        imported_count: 0,
                        error_count: 1,
                        errors: vec![format!("表 {} 不存在，请使用 CreateNew 模式创建表", table_name)],
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
                Err(e) => {
                    return ImportResult {
                        success: false,
                        imported_count: 0,
                        error_count: 1,
                        errors: vec![format!("检查表是否存在失败: {}", e)],
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
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

    let total_count = layer.feature_count() as i64;
    // 达梦数据库单条插入，批次大小用于进度更新
    let batch_size = if total_count <= 500 { 10 } else if total_count <= 10000 { 100 } else { 300 };

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
    info!("达梦导入 - 总记录数: {}", total_count);

    // 开始事务
    let mut tx = match conn.begin_transaction() {
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

    let mut batch: Vec<(String, Vec<DamengFieldValue>)> = Vec::with_capacity(batch_size);
    let mut imported_count: i64 = 0;
    let mut error_count: i64 = 0;
    let mut errors: Vec<String> = Vec::new();

    for feat in layer.features() {
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

        let geometry = match feat.geometry() {
            Some(g) => g,
            None => { error_count += 1; continue; }
        };

        let wkt = match geometry.wkt() {
            Ok(w) => w,
            Err(_) => { error_count += 1; continue; }
        };

        let mut row_data: Vec<DamengFieldValue> = Vec::with_capacity(field_names.len());
        for field_name in &field_names {
            let field_idx = field_index_map.get(field_name).copied();
            let field_value = match field_idx {
                Some(idx) if idx < 1000 => {
                    match feat.field(idx) {
                        Ok(Some(field)) => {
                            match field {
                                gdal::vector::FieldValue::IntegerValue(v) => DamengFieldValue::Integer(v),
                                gdal::vector::FieldValue::Integer64Value(v) => DamengFieldValue::Integer64(v),
                                gdal::vector::FieldValue::RealValue(v) => DamengFieldValue::Real(v as f32),
                                gdal::vector::FieldValue::StringValue(v) => {
                                    let cleaned: String = v.chars()
                                        .filter(|c| *c != '\0' && *c != '\r')
                                        .map(|c| if c == '\n' { ' ' } else { c })
                                        .collect();
                                    DamengFieldValue::Text(cleaned)
                                }
                                _ => DamengFieldValue::Null,
                            }
                        }
                        _ => DamengFieldValue::Null,
                    }
                }
                _ => DamengFieldValue::Null,
            };
            row_data.push(field_value);
        }

        batch.push((wkt, row_data));

        // 达梦数据库单条插入，按批次提交进度
        if batch.len() >= batch_size {
            match tx.batch_insert(table_name, &field_names, srs, &batch) {
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
        match tx.batch_insert(table_name, &field_names, srs, &batch) {
            Ok(count) => imported_count += count as i64,
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
        Ok(_) => info!("事务提交成功"),
        Err(e) => {
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
    info!("达梦导入完成，总耗时: {}ms", duration.as_millis());

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

    let handle = app_handle.clone();
    let app_handle_for_complete = app_handle.clone();
    std::thread::spawn(move || {
        info!("后台线程启动");

        // 使用 catch_unwind 捕获 panic
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            do_import_in_background(config, handle)
        }));

        match result {
            Ok(import_result) => {
                info!("导入完成，发送结果事件");
                let _ = app_handle_for_complete.emit("import-complete", import_result);
            }
            Err(panic_info) => {
                error!("导入过程中发生 panic: {:?}", panic_info);
                let error_result = ImportResult {
                    success: false,
                    imported_count: 0,
                    error_count: 1,
                    errors: vec!["导入过程中发生内部错误，请查看日志".to_string()],
                    duration_ms: 0,
                };
                let _ = app_handle_for_complete.emit("import-complete", error_result);
            }
        }
    });

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