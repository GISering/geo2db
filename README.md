# Spatial Import Tool

> 高性能空间数据导入工具，支持 Shapefile、GeoPackage、GeoJSON、KML 等多种格式导入 PostgreSQL/PostGIS 和达梦数据库

![Tauri](https://img.shields.io/badge/Tauri-2-24c8d8?style=flat-square&logo=tauri)
![React](https://img.shields.io/badge/React-19-61dafb?style=flat-square&logo=react)
![Rust](https://img.shields.io/badge/Rust-1.70+-f74c00?style=flat-square&logo=rust)
![TypeScript](https://img.shields.io/badge/TypeScript-5.8-3178c6?style=flat-square&logo=typescript)
![GDAL](https://img.shields.io/badge/GDAL-0.19-5cb85c?style=flat-square)

---

## 目录

- [核心功能](#核心功能)
- [技术架构](#技术架构)
- [代码结构](#代码结构)
- [调用流程](#调用流程)
- [数据库支持](#数据库支持)
- [技术栈详解](#技术栈详解)
- [核心设计模式](#核心设计模式)
- [快速开始](#快速开始)

---

## 核心功能

### 📁 多格式空间数据支持

支持 Shapefile (.shp)、GeoPackage (.gpkg)、GeoJSON、KML 等主流空间数据格式，基于 GDAL 库实现强大的格式兼容性。

### 🗄️ 多数据库支持

支持 PostgreSQL/PostGIS 和达梦数据库，采用 SQL 方言抽象设计，易于扩展更多数据库类型。

### ⚡ 高性能批量导入

智能批次大小调整，事务批量提交，支持大数据量高效导入。动态批次策略优化导入性能。

### 🔄 灵活导入模式

三种导入模式：创建新表、追加数据、替换重建。自动处理空间参考系统 (SRS) 转换。

### 📊 实时进度反馈

异步导入架构，实时进度事件推送。支持取消操作，导入结果详细统计。

### 💾 配置持久化

数据库连接配置本地持久化存储，支持多配置管理，快速切换数据源。

---

## 技术架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                        前端层 - React + TypeScript                    │
├─────────────────────┬─────────────────────┬─────────────────────────┤
│      状态管理        │       UI 组件        │        UI 框架          │
├─────────────────────┼─────────────────────┼─────────────────────────┤
│ • useImport Hook    │ • FileSelector      │ • Ant Design 6.x        │
│ • useState          │ • DbConfigList      │ • Vite 构建工具          │
│ • Tauri Events      │ • ImportConfig      │ • CSS Modules           │
│                     │ • Progress          │                         │
└─────────────────────┴─────────────────────┴─────────────────────────┘
                                ↓ Tauri IPC ↓
┌─────────────────────────────────────────────────────────────────────┐
│                        桥接层 - Tauri Commands                       │
├─────────────────────┬─────────────────────┬─────────────────────────┤
│      文件命令        │      数据库命令      │        导入命令          │
├─────────────────────┼─────────────────────┼─────────────────────────┤
│ • list_files        │ • test_connection   │ • import_file           │
│ • list_layers       │ • save_config       │ • cancel_import         │
│ • get_file_info     │ • load_config       │ • get_import_progress   │
│ • get_supported_    │ • delete_config     │                         │
│   drivers           │                     │                         │
└─────────────────────┴─────────────────────┴─────────────────────────┘
                                ↓ ↓
┌─────────────────────────────────────────────────────────────────────┐
│                           后端层 - Rust                              │
├─────────────────────┬─────────────────────┬─────────────────────────┤
│      GDAL 模块       │      数据库模块      │        数据模型          │
├─────────────────────┼─────────────────────┼─────────────────────────┤
│ • 空间文件读取       │ • SqlDialect Trait  │ • DbConfig              │
│ • 几何图形解析       │ • PostgreSQL 实现    │ • FileInfo              │
│ • 字段类型映射       │ • 达梦数据库实现      │ • ImportConfig          │
│ • 空间参考处理       │ • 事务管理          │ • ImportResult          │
└─────────────────────┴─────────────────────┴─────────────────────────┘
```

---

## 代码结构

### 前端目录 (src/)

```
src/
├── App.tsx                 # 主应用组件
├── main.tsx                # 入口文件
├── components/
│   ├── Header.tsx          # 顶部导航
│   ├── FileSelector.tsx    # 文件选择器
│   ├── DbConfig.tsx        # 数据库配置表单
│   ├── DbConfigList.tsx    # 配置列表管理
│   ├── ImportConfig.tsx    # 导入配置面板
│   └── Progress.tsx        # 进度显示
├── hooks/
│   └── useImport.ts        # 核心业务 Hook
└── types/
    └── index.ts            # 类型定义
```

### 后端目录 (src-tauri/src/)

```
src-tauri/src/
├── main.rs                 # Tauri 应用入口
├── lib.rs                  # 库入口 & 命令注册
├── commands/
│   ├── file_commands.rs    # 文件操作命令
│   ├── db_commands.rs      # 数据库操作命令
│   └── import_commands.rs  # 导入执行命令
├── database/
│   ├── mod.rs              # 数据库模块入口
│   ├── traits.rs           # SqlDialect Trait
│   ├── postgres.rs         # PostgreSQL 实现
│   ├── dameng.rs           # 达梦数据库实现
│   └── dialects/
│       ├── postgres.rs     # PG SQL 方言
│       └── dameng.rs       # 达梦 SQL 方言
├── gdal/
│   └── mod.rs              # GDAL 封装
└── models/
    └── mod.rs              # 数据模型定义
```

---

## 调用流程

```
┌──────────────────────────────────────────────────────────────────┐
│  1. 用户选择空间文件                                               │
│     Tauri Dialog → list_files → GDAL 读取文件元数据                │
└──────────────────────────────────────────────────────────────────┘
                                ↓
┌──────────────────────────────────────────────────────────────────┐
│  2. 配置数据源连接                                                 │
│     填写连接信息 → test_connection → 成功后自动保存配置             │
└──────────────────────────────────────────────────────────────────┘
                                ↓
┌──────────────────────────────────────────────────────────────────┐
│  3. 设置导入参数                                                   │
│     表名、空间参考系统(SRS)、导入模式(CreateNew/Append/Replace)     │
└──────────────────────────────────────────────────────────────────┘
                                ↓
┌──────────────────────────────────────────────────────────────────┐
│  4. 执行异步导入                                                   │
│     import_file → 后台线程执行 → Tauri Events 推送实时进度          │
└──────────────────────────────────────────────────────────────────┘
                                ↓
┌──────────────────────────────────────────────────────────────────┐
│  5. 批量数据处理                                                   │
│     GDAL 遍历 Feature → 转 WKT → SqlDialect 生成 SQL → 批量 INSERT │
└──────────────────────────────────────────────────────────────────┘
                                ↓
┌──────────────────────────────────────────────────────────────────┐
│  6. 返回导入结果                                                   │
│     import-complete 事件 → 显示成功/失败统计、耗时信息              │
└──────────────────────────────────────────────────────────────────┘
```

---

## 数据库支持

### 🐘 PostgreSQL / PostGIS

最成熟的空间数据库解决方案

| 特性 | 说明 |
|------|------|
| PostGIS 扩展 | 原生空间数据支持 |
| ST_GeomFromText | WKT 转几何函数 |
| 批量 INSERT | 高性能数据导入 |
| 事务批量提交 | 数据一致性保障 |
| Geometry 类型 | 标准空间字段类型 |
| SERIAL 主键 | 自动递增主键 |

**技术栈**: `rust-postgres` + `postgis`

---

### 🏛️ 达梦数据库 (DM)

国产数据库，支持空间扩展

| 特性 | 说明 |
|------|------|
| DMGEO 空间扩展 | 达梦空间数据支持 |
| DMI_GEOMETRY 类型 | 空间字段类型 |
| IDENTITY 主键 | 自动递增主键 |
| ODBC 驱动 | 标准 JDBC/ODBC 连接 |
| PreparedStatement | 批量插入支持 |
| 自定义 SRID | 空间参考系统支持 |

**技术栈**: `odbc-api` + `DMGEO`

---

## 技术栈详解

| 技术 | 说明 |
|------|------|
| **Tauri 2** | Rust 构建的桌面应用框架，轻量高性能 |
| **React 19** | 最新版 React，组件化 UI 开发 |
| **Rust** | 内存安全的系统编程语言，零成本抽象 |
| **TypeScript 5.8** | 类型安全的 JavaScript 超集 |
| **GDAL 0.19** | 地理空间数据抽象库，支持多种格式 |
| **postgres / postgis** | Rust PostgreSQL 驱动与空间扩展 |
| **odbc-api** | Rust ODBC 驱动，支持多种数据库 |
| **Tokio** | Rust 异步运行时，高性能并发 |

---

## 核心设计模式

### 🔧 SQL 方言抽象 (SqlDialect Trait)

统一抽象不同数据库的 SQL 语法差异，便于扩展新的数据库类型。

```rust
pub trait SqlDialect: Send + Sync {
    fn db_type(&self) -> DbType;
    fn geometry_type_name(&self) -> &str;      // 空间类型名称
    fn geom_from_wkt(&self, wkt: &str, srid: &str) -> String;  // WKT 转几何
    fn map_field_type(&self, gdal_type: &str) -> &str;  // 字段类型映射
    fn param_placeholder(&self, idx: usize) -> String;  // 参数占位符
    // ...
}
```

### 🎯 自定义 Hook 模式

`useImport` Hook 集中管理所有业务状态，包括文件选择、数据库配置、导入执行、事件监听等。

### 📡 事件驱动架构

- `import-progress` - 实时进度更新
- `import-complete` - 导入完成通知
- 异步非阻塞用户体验
- 支持取消操作

---

## 快速开始

### 环境要求

- Node.js 18+
- Rust 1.70+
- GDAL 库

### 安装依赖

```bash
# 安装前端依赖
npm install

# Cargo 需要在 PATH 中
export PATH="$HOME/.cargo/bin:$PATH"
```

### 开发运行

```bash
# 启动开发服务器（同时运行前端和 Tauri 后端）
npm run tauri dev
```

### 生产构建

```bash
# 构建生产版本
npm run tauri build
```

---

## 导入模式说明

| 模式 | 说明 |
|------|------|
| `CreateNew` | 创建新表，如果表已存在则报错 |
| `Append` | 追加数据到现有表 |
| `Replace` | 删除并重建表，然后导入数据 |

---

## 配置存储

数据库配置持久化存储在：

```
~/.config/spatial-import-tool/db_config.json
```

---

## 推荐 IDE 配置

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

---

## License

MIT

---

<p align="center">
  <b>Spatial Import Tool v0.1.0</b><br>
  基于 Tauri 2 + React + Rust 构建<br><br>
  支持格式: Shapefile, GeoPackage, GeoJSON, KML<br>
  数据库: PostgreSQL/PostGIS, 达梦
</p>