use crate::gdal::{GdalHandler, LayerInfo};
use crate::models::FileInfo;

#[tauri::command]
pub async fn list_files(paths: Vec<String>) -> Result<Vec<FileInfo>, String> {
    let mut files = Vec::new();

    for path in paths {
        match GdalHandler::get_file_info(&path, None) {
            Ok(info) => files.push(info),
            Err(e) => {
                log::warn!("读取文件失败 {}: {}", path, e);
            }
        }
    }

    Ok(files)
}

#[tauri::command]
pub async fn get_file_info(path: String, layer_name: Option<String>) -> Result<FileInfo, String> {
    GdalHandler::get_file_info(&path, layer_name.as_deref())
}

#[tauri::command]
pub async fn list_layers(path: String) -> Result<Vec<LayerInfo>, String> {
    GdalHandler::list_layers(&path)
}

#[tauri::command]
pub fn get_supported_drivers() -> Vec<String> {
    GdalHandler::get_supported_drivers()
}