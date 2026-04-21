<p align="center">
  <img src="assets/branding/relora-terminal-wordmark.svg" alt="Relora" width="560" />
</p>

<p align="center">
  <strong>基于 Rust 和 ratatui 构建的键盘优先终端数据库工作台。</strong>
</p>

<p align="center">
  <a href="https://github.com/murongg/relora/actions/workflows/ci.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/murongg/relora/ci.yml?branch=main&style=flat-square&logo=githubactions&label=ci" alt="CI 状态" />
  </a>
  <a href="https://github.com/murongg/relora/releases">
    <img src="https://img.shields.io/github/v/release/murongg/relora?style=flat-square&label=release" alt="最新版本" />
  </a>
  <a href="https://www.npmjs.com/package/relora">
    <img src="https://img.shields.io/npm/v/relora?style=flat-square&logo=npm" alt="npm 版本" />
  </a>
  <a href="LICENSE">
    <img src="https://img.shields.io/github/license/murongg/relora?style=flat-square" alt="MIT 许可证" />
  </a>
  <img src="https://img.shields.io/badge/rust-1.85%2B-dea584?style=flat-square&logo=rust" alt="Rust 1.85+" />
</p>

<p align="center">
  <a href="README.md">English</a> | 简体中文
</p>

<p align="center">
  <img src="assets/screenshots/db.png" alt="Relora 工作区截图，展示了资源树、标签页和数据预览表格。" width="1200" />
</p>

# Relora

Relora 是一个终端数据库工作台，适合那些不想为了看表、跑 SQL、做轻量编辑就切去打开 GUI 客户端的人。

它把最常见的数据库操作收在一个终端工作区里：

- 打开一个或多个保存过的数据库连接
- 浏览数据库、schema、表和结构信息
- 在高信息密度但键盘友好的表格里预览数据
- 直接编写并执行 SQL
- 先 staged，再提交行级编辑

## 为什么是 Relora

Relora 想解决的是“任务很轻，但 GUI 很重”的那种不协调感。

- 启动快
- 键盘优先
- 一个 workspace 里处理多连接
- 使用 sidecar driver，而不是把所有数据库客户端都塞进主程序

## 功能特性

- **多连接工作区**：在同一个 session 里打开并浏览一个或多个保存的连接。
- **高密度数据预览**：支持分页、过滤、复制和 row detail，适合快速检查数据。
- **内置 SQL tab**：直接写 SQL、执行当前 statement、复用历史记录、查看结果集。
- **结构视图**：不离开工作区就能查看字段和对象元数据。
- **staged 编辑**：先预览生成的 SQL，再提交行级修改。
- **sidecar driver 架构**：把 PostgreSQL、MySQL / MariaDB、SQLite 支持放在主 TUI 之外。

## 安装

### npm

```bash
npm install -g relora
relora
```

也可以不全局安装，直接运行：

```bash
npx relora
```

### curl

```bash
curl -fsSL https://raw.githubusercontent.com/murongg/relora/main/scripts/install.sh | sh
```

### 源码

```bash
cargo run -p relora
```

## 快速开始

### 1. 启动工作区

```bash
relora
```

或者直接打开一个连接：

```bash
relora --url postgresql://localhost:5432/postgres
```

### 2. 添加保存连接

在启动页中：

- `a` 新建连接
- `e` 编辑当前连接
- `t` 测试当前连接
- `Enter` 启动当前连接

保存的连接默认写在：

```text
~/.config/relora/connections.json
```

### 3. 在工作区里操作

- 在左侧资源树里选择数据库对象
- 用 `F2`、`F3`、`F4` 或 `Alt-1`、`Alt-2`、`Alt-3` 切换 `Data`、`SQL`、`Structure`
- 用 `/` 过滤预览数据
- 用 `F5` 或 `Ctrl-Enter` 执行当前 SQL statement
- 在数据表格中按 `i` 发起 staged 编辑

## 当前支持的数据库

Relora 当前支持：

- PostgreSQL
- MySQL / MariaDB
- SQLite

更细的支持状态和待对齐项见 [docs/feature-parity.md](docs/feature-parity.md)。

数据库能力通过 sidecar binary 提供：

- `relora-driver-postgres`
- `relora-driver-mysql`
- `relora-driver-sqlite`

## 你能得到什么

- 多连接启动页
- 一个终端视图里的 `Data`、`SQL`、`Structure` 三个 tab
- SQL history、自动补全和按 statement 执行
- 行预览、复制和快速过滤
- PostgreSQL `EXPLAIN` / `EXPLAIN ANALYZE`
- staged CRUD，提交前先预览 SQL

## 性能基线

Relora 现在带有一套基于 `criterion` 的性能基准，覆盖工作区状态热路径和 TUI 渲染热路径。当前在维护者机器上通过 `--quick` 得到的基线大致是：

| Benchmark | 基线 |
| --- | --- |
| `workspace_bootstrap_large_catalog` | `~411 µs` |
| `workspace_cancel_inflight_preview` | `~11.6 µs` |
| `workspace_scroll_wide_preview_columns` | `~177 µs` |
| `workspace_switch_sql_result_sets` | `~491 µs` |
| `render_workspace_data_tab_dense_grid` | `~1.74 ms` |
| `render_workspace_sql_tab_result_grid` | `~1.36 ms` |
| `render_workspace_row_inspector_long_text` | `~528 µs` |

本地运行方式：

```bash
cargo bench -p relora-app --bench workspace_hot_paths -- --quick
cargo bench -p relora --bench tui_render_hot_paths -- --quick
```

这些数值主要用于后续回归对比，不代表所有机器上的固定性能承诺。

## 快捷键

### 启动页

| 键位 | 作用 |
| --- | --- |
| `j` / `k` | 在保存的连接之间移动 |
| `Space` | 标记或取消标记连接，用于多连接启动 |
| `a` | 新建连接 |
| `e` | 编辑当前连接 |
| `d` | 删除当前连接 |
| `t` | 测试当前连接 |
| `Enter` | 启动当前连接 |
| `q` / `Esc` | 退出或取消 |

### 全局

| 键位 | 作用 |
| --- | --- |
| `Tab` / `Shift-Tab` | 在 pane 之间轮转焦点 |
| `F2` 或 `Alt-1` | 打开 `Data` |
| `F3` 或 `Alt-2` | 打开 `SQL` |
| `F4` 或 `Alt-3` | 打开 `Structure` |
| `Ctrl-P` | 打开命令面板 |
| `F10` 或 `Ctrl-R` | 打开 SQL history |

### 资源树与浏览区

| 键位 | 作用 |
| --- | --- |
| `j` / `k` | 移动选择 |
| `h` / `l` | 折叠或展开 |
| `Enter` / `Space` | 切换当前节点 |
| `/` | 打开数据过滤 |
| `e` | 打开 SQL 编辑器 |
| `s` / `i` / `u` / `d` | 插入 `SELECT` / `INSERT` / `UPDATE` / `DELETE` 模板 |
| `r` | 刷新当前选择 |
| `c` | 取消运行中的任务 |
| `q` / `Esc` | 退出工作区或返回焦点 |

### 数据表格

| 键位 | 作用 |
| --- | --- |
| `j` / `k` | 移动行 |
| `h` / `l` | 移动列 |
| `Enter` | 打开 row inspector |
| `/` | 过滤当前预览 |
| `n` / `p` | 下一页 / 上一页预览 |
| `y` / `Y` | 复制当前 row / cell |
| `w` | 复制自动生成的 `WHERE` 条件 |
| `i` | 发起 staged cell 编辑 |
| `[` / `]` / `=` | 缩小、扩大或重置列宽 |
| `f` / `F` | 冻结列或清除冻结列 |

### SQL 编辑器

| 键位 | 作用 |
| --- | --- |
| `Ctrl-Enter` 或 `F5` | 执行当前 statement |
| `Ctrl-T` | 新建 SQL tab |
| `Ctrl-W` | 关闭 SQL tab |
| `F6` / `F7` | 上一个 / 下一个 SQL tab |
| `F8` / `F9` | 上一个 / 下一个结果集 |
| `F10` 或 `Ctrl-R` | 打开 SQL history |
| `F11` / `F12` | `EXPLAIN` / `EXPLAIN ANALYZE` |
| `Ctrl-G` | 提交 staged CRUD |

### Row Inspector

| 键位 | 作用 |
| --- | --- |
| `Tab` | 在 inspector pane 间切换 |
| `j` / `k` | 移动或滚动 |
| `PgUp` / `PgDn` | 按页滚动预览 |
| `Ctrl-U` / `Ctrl-D` | 更快滚动 |
| `y` / `Y` | 复制当前值 |
| `i` | 从当前字段进入编辑 |
| `f` | 切换原始值 / 格式化显示 |
| `q` / `Esc` | 关闭 inspector |
