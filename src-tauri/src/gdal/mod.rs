use crate::models::{FieldInfo, FileInfo, SpatialRefInfo};
use gdal::Dataset;
use gdal::vector::{LayerAccess, OGRFieldType, OGRwkbGeometryType};
use serde::{Deserialize, Serialize};

/// Convert OGR field type to readable string
fn field_type_name(ty: OGRFieldType::Type) -> String {
    match ty {
        OGRFieldType::OFTString => "String".to_string(),
        OGRFieldType::OFTInteger => "Integer".to_string(),
        OGRFieldType::OFTIntegerList => "IntegerList".to_string(),
        OGRFieldType::OFTReal => "Real".to_string(),
        OGRFieldType::OFTRealList => "RealList".to_string(),
        OGRFieldType::OFTStringList => "StringList".to_string(),
        OGRFieldType::OFTWideString => "WideString".to_string(),
        OGRFieldType::OFTWideStringList => "WideStringList".to_string(),
        OGRFieldType::OFTBinary => "Binary".to_string(),
        OGRFieldType::OFTDate => "Date".to_string(),
        OGRFieldType::OFTTime => "Time".to_string(),
        OGRFieldType::OFTDateTime => "DateTime".to_string(),
        OGRFieldType::OFTInteger64 => "Integer64".to_string(),
        OGRFieldType::OFTInteger64List => "Integer64List".to_string(),
        _ => format!("Unknown({})", ty),
    }
}

/// Convert OGR geometry type to readable string
fn geometry_type_name(ty: OGRwkbGeometryType::Type) -> String {
    match ty {
        OGRwkbGeometryType::wkbPoint => "Point".to_string(),
        OGRwkbGeometryType::wkbLineString => "LineString".to_string(),
        OGRwkbGeometryType::wkbPolygon => "Polygon".to_string(),
        OGRwkbGeometryType::wkbMultiPoint => "MultiPoint".to_string(),
        OGRwkbGeometryType::wkbMultiLineString => "MultiLineString".to_string(),
        OGRwkbGeometryType::wkbMultiPolygon => "MultiPolygon".to_string(),
        OGRwkbGeometryType::wkbGeometryCollection => "GeometryCollection".to_string(),
        OGRwkbGeometryType::wkbCircularString => "CircularString".to_string(),
        OGRwkbGeometryType::wkbCompoundCurve => "CompoundCurve".to_string(),
        OGRwkbGeometryType::wkbCurvePolygon => "CurvePolygon".to_string(),
        OGRwkbGeometryType::wkbMultiCurve => "MultiCurve".to_string(),
        OGRwkbGeometryType::wkbMultiSurface => "MultiSurface".to_string(),
        OGRwkbGeometryType::wkbCurve => "Curve".to_string(),
        OGRwkbGeometryType::wkbSurface => "Surface".to_string(),
        OGRwkbGeometryType::wkbPolyhedralSurface => "PolyhedralSurface".to_string(),
        OGRwkbGeometryType::wkbTIN => "TIN".to_string(),
        OGRwkbGeometryType::wkbTriangle => "Triangle".to_string(),
        OGRwkbGeometryType::wkbNone => "None".to_string(),
        OGRwkbGeometryType::wkbUnknown => "Unknown".to_string(),
        _ => format!("Unknown({})", ty),
    }
}

/// 图层信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerInfo {
    pub name: String,
    pub feature_count: i64,
}

pub struct GdalHandler;

impl GdalHandler {
    pub fn get_file_info(path: &str, layer_name: Option<&str>) -> Result<FileInfo, String> {
        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let format = Self::detect_format(path);

        // Open dataset using GDAL
        let dataset = Dataset::open(path).map_err(|e| format!("无法打开文件: {}", e))?;

        // Get the specified layer or first layer
        let layer = if let Some(layer_name) = layer_name {
            dataset
                .layer_by_name(layer_name)
                .map_err(|e| format!("找不到图层 {}: {}", layer_name, e))?
        } else {
            let mut layers = dataset.layers();
            layers.next().ok_or("没有找到图层")?
        };

        let layer_name = layer.name().to_string();

        // Get feature count
        let feature_count = layer.feature_count() as i64;

        // Get fields information
        let defn = layer.defn();
        let mut fields = Vec::new();
        for field in defn.fields() {
            fields.push(FieldInfo {
                name: field.name().to_string(),
                field_type: field_type_name(field.field_type()),
            });
        }

        // Get geometry type
        let geometry_type = if let Some(geom_field) = defn.geom_fields().next() {
            geometry_type_name(geom_field.field_type())
        } else {
            "Unknown".to_string()
        };

        // Get spatial reference information
        let srs = dataset
            .spatial_ref()
            .map_err(|e| format!("获取坐标系统失败: {}", e))?;

        let epsg = srs.auth_code().ok().unwrap_or(0);

        let proj4 = srs.to_proj4().ok();
        let wkt = srs.to_wkt().ok();

        Ok(FileInfo {
            path: path.to_string(),
            name: name.clone(),
            format,
            layer_name,
            feature_count,
            geometry_type,
            fields,
            srs: if epsg > 0 || proj4.is_some() || wkt.is_some() {
                Some(SpatialRefInfo {
                    epsg,
                    proj4,
                    wkt,
                })
            } else {
                None
            },
        })
    }

    fn detect_format(path: &str) -> String {
        let path = std::path::Path::new(path);
        match path.extension().and_then(|e| e.to_str()) {
            Some("shp") => "Shapefile".to_string(),
            Some("gpkg") => "GeoPackage".to_string(),
            Some("geojson") => "GeoJSON".to_string(),
            Some("kml") => "KML".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    pub fn get_supported_drivers() -> Vec<String> {
        vec![
            "ESRI Shapefile".to_string(),
            "GeoPackage".to_string(),
            "GeoJSON".to_string(),
            "KML".to_string(),
        ]
    }

    /// 列出文件中的所有图层
    pub fn list_layers(path: &str) -> Result<Vec<LayerInfo>, String> {
        let dataset = Dataset::open(path).map_err(|e| format!("无法打开文件: {}", e))?;

        let mut layers_info = Vec::new();
        for layer in dataset.layers() {
            layers_info.push(LayerInfo {
                name: layer.name().to_string(),
                feature_count: layer.feature_count() as i64,
            });
        }

        Ok(layers_info)
    }
}