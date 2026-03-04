<!--
 * @Author: 贾曲 1151521238@qq.com
 * @Date: 2026-03-02 22:30:41
 * @LastEditors: 贾曲 1151521238@qq.com
 * @LastEditTime: 2026-03-02 22:34:45
 * @FilePath: /test-claude/CLAUDE.md
 * @Description: 这是默认设置,请设置`customMade`, 打开koroFileHeader查看配置 进行设置: https://github.com/OBKoro1/koro1FileHeader/wiki/%E9%85%8D%E7%BD%AE
-->
# CLAUDE.md

本文档为 Claude Code (claude.ai/code) 在本项目中工作时提供指导。

## 项目概述

**Spatial Import Tool** - 一个用于将空间数据（shapefile、GeoPackage、GeoJSON、KML）导入 PostgreSQL/PostGIS 数据库的桌面应用程序。基于 Tauri 2 + React + TypeScript + Rust 构建。

## 常用命令

```bash
# 启动开发服务器（同时运行前端和 Tauri 后端）
npm run tauri dev

# 生产环境构建
npm run tauri build

# 仅构建前端
npm run build

# 仅运行前端开发服务器
npm run dev
```

## 架构

### 前端 (React + TypeScript)
- **状态管理**: 自定义 Hook `src/hooks/useImport.ts` 管理所有应用状态
- **组件**:
  - `FileSelector.tsx` - 文件选择和图层浏览
  - `DbConfig.tsx` - 数据库连接配置
  - `ImportConfig.tsx` - 导入设置（表名、SRS、导入模式）
  - `Progress.tsx` - 导入进度显示
- **类型定义**: `src/types/index.ts` - 共享的 TypeScript 接口

### 后端 (Rust/Tauri)
- **命令模块**:
  - `file_commands.rs` - 文件操作（list_files、list_layers、get_file_info）
  - `db_commands.rs` - 数据库操作（test_connection、save_config、load_config）
  - `import_commands.rs` - 导入逻辑（import_file、get_import_progress）
- **数据库**:
  - `postgres.rs` - PostgreSQL/PostGIS 连接和配置持久化
- **GDAL**:
  - `gdal/mod.rs` - 使用 GDAL 库读取空间数据
- **模型**:
  - `models/mod.rs` - 数据结构（DbConfig、FileInfo、ImportConfig、ImportResult 等）

### 关键技术
- **Tauri 2** - 桌面应用框架
- **GDAL 0.19** - 空间数据格式处理
- **PostgreSQL/postgis** - 数据库驱动
- **Tokio** - Rust 异步运行时
- **Vite** - 前端构建工具

## 数据库配置

数据库配置持久化到 `~/.config/spatial-import-tool/db_config.json`。

## 导入模式

- `CreateNew` - 创建新表
- `Append` - 追加到现有表
- `Replace` - 删除并重建表

## 注意事项

- Cargo 需要在 PATH 中：运行 Tauri 命令前需要执行 `export PATH="$HOME/.cargo/bin:$PATH"`
- 应用使用 GDAL 读取空间文件，并将几何转换为 WKT 格式插入 PostGIS

## 测试数据和数据库
### 使用文件 `/Users/oozie/数据/测试数据/矢量数据/2021成果_2000/全市dm2021_2000.shp`
### 数据库
- 47.100.102.7:10001
- 数据库：work_shop
- 用户名：agriview_work_shop
- 密码：123QWE456rty!