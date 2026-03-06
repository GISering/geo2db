// 数据库类型
export type DbType = 'PostgreSQL' | 'Dameng';

// 数据库配置
export interface DbConfig {
  db_type: DbType;
  host: string;
  port: number;
  database: string;
  username: string;
  password: string;
}

// 带名称的数据库配置（用于保存多个配置）
// 注意：Rust 使用 #[serde(flatten)]，所以 config 字段直接展开
export interface NamedDbConfig extends DbConfig {
  name: string;
}

// 数据库配置列表
export interface DbConfigList {
  configs: NamedDbConfig[];
  active_config: string | null;  // 当前选中的配置名称 (snake_case 以匹配 Rust)
}

// 文件信息
export interface FileInfo {
  path: string;
  name: string;
  format: string;
  layer_name: string;
  feature_count: number;
  geometry_type: string;
  fields: FieldInfo[];
  srs: SpatialRefInfo | null;
}

// 字段信息
export interface FieldInfo {
  name: string;
  field_type: string;
}

// 图层信息
export interface LayerInfo {
  name: string;
  feature_count: number;
}

// 空间参考信息
export interface SpatialRefInfo {
  epsg: number;
  proj4: string | null;
  wkt: string | null;
}

// 导入模式
export type ImportMode = 'CreateNew' | 'Append' | 'Replace';

// 导入配置
export interface ImportConfig {
  db_config: DbConfig;
  file_path: string;
  layer_name: string | null;
  table_name: string;
  srs: string | null;
  import_mode: ImportMode;
  field_mapping: Record<string, string> | null;
}

// 导入进度
export interface ImportProgress {
  current: number;
  total: number;
  status: string;
  message: string;
}

// 导入结果
export interface ImportResult {
  success: boolean;
  imported_count: number;
  error_count: number;
  errors: string[];
  duration_ms: number;
}

// 连接测试结果
export interface ConnectionTestResult {
  success: boolean;
  message: string;
  server_version: string | null;
}

// 达梦驱动状态
export interface DamengDriverStatus {
  installed: boolean;
  message: string;
}

// 应用步骤
export type AppStep = 'files' | 'import' | 'progress';