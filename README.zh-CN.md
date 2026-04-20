# Relora

[English](README.md) | 简体中文

Relora 是一个基于 Rust 和 `ratatui` 构建的终端数据库工作台。

它面向键盘优先的数据库工作流：在终端里同时完成多连接管理、结构浏览、数据预览、SQL 执行，以及相对安全的 staged 编辑。

## Relora 是做什么的

Relora 想替代的是“为了看一张表、跑一条 SQL，就切去打开 GUI 客户端”的那类工作流。

它更适合这样的人：

- 偏好终端原生工具
- 需要管理多个数据库连接
- 希望把浏览、预览、编辑放在一个工作区里
- 在意性能和更轻量的架构

## 功能概览

### 多连接工作区

- 保存并管理多个命名连接
- 在同一个 workspace 里打开一个或多个连接
- 通过左侧资源树浏览数据库、schema 与对象分组

### 数据浏览

- 在 `Data` tab 预览表类对象数据
- 支持预览分页
- 在 `Structure` tab 查看字段与元数据
- 用 row inspector 查看宽表和长文本
- 复制当前 cell、当前 row 或自动生成的 `WHERE` 条件

### SQL 工作流

- 内置 SQL 编辑器
- 默认只执行光标所在 statement
- SQL history 支持搜索和重跑
- SQL 自动补全支持关键词、对象名和列名
- 支持多个 SQL tab 和多个结果集
- PostgreSQL `EXPLAIN` / `EXPLAIN ANALYZE`

### 更安全的编辑流程

- 在 `Data` tab 中快速过滤数据
- 生成 `SELECT`、`INSERT`、`UPDATE`、`DELETE` 模板
- staged CRUD：先预览 SQL，再事务提交

### 运行时模型

- 每个连接独立后台 worker
- 预览刷新、结构加载、SQL 执行都是异步的
- 支持任务去重、取消和优先级调度

## 当前支持的数据库

Relora 当前支持：

- PostgreSQL
- MySQL / MariaDB
- SQLite

这些能力都通过 external sidecar driver 提供：

- `relora-driver-postgres`
- `relora-driver-mysql`
- `relora-driver-sqlite`

Relora 主程序本身不会直接链接数据库客户端 driver。

## 安装方式

### npm

对终端用户来说，最简单的安装方式是 npm：

```bash
npm install -g relora
relora
```

npm 包会在安装阶段从 GitHub Releases 下载当前平台对应的预编译 Relora bundle，其中包含：

- `relora`
- `relora-driver-postgres`
- `relora-driver-mysql`
- `relora-driver-sqlite`

也可以不全局安装，直接运行：

```bash
npx relora
```

### curl（macOS / Linux）

如果你更喜欢一条 shell 命令安装：

```bash
curl -fsSL https://raw.githubusercontent.com/murongg/relora/main/scripts/install.sh | sh
```

安装脚本默认会把当前平台匹配的预编译 release bundle 下载到 `~/.local/bin`。

常用覆盖参数：

```bash
curl -fsSL https://raw.githubusercontent.com/murongg/relora/main/scripts/install.sh | RELORA_VERSION=0.1.0 sh
curl -fsSL https://raw.githubusercontent.com/murongg/relora/main/scripts/install.sh | RELORA_INSTALL_DIR=/usr/local/bin sh
```

### 源码方式

对贡献者和本地开发来说，源码启动方式仍然是：

```bash
cargo run -p relora
```

## 怎么使用 Relora

### 1. 启动 Relora

打开启动页：

```bash
relora
```

或者从源码启动：

```bash
cargo run -p relora
```

直接打开单个连接：

```bash
cargo run -p relora -- --url postgresql://postgres:postgres@localhost:5432/postgres
```

一次打开多个命名连接：

```bash
cargo run -p relora -- \
  --connection pg=postgresql://postgres:postgres@localhost:5432/postgres \
  --connection analytics=postgresql://postgres:postgres@localhost:5432/analytics
```

也可以用环境变量：

```bash
export RELORA_DATABASE_URL=postgresql://postgres:postgres@localhost:5432/postgres
cargo run -p relora
```

或者：

```bash
export RELORA_CONNECTIONS='pg=postgresql://postgres:postgres@localhost:5432/postgres;analytics=postgresql://postgres:postgres@localhost:5432/analytics'
cargo run -p relora
```

保存的连接默认写入：

```text
~/.config/relora/connections.json
```

### 2. 添加或编辑连接

如果你从 launcher 启动，按 `a` 可以新增连接。

表单支持这些字段：

- Driver
- Host / SQLite path
- Port
- Database
- User
- Password
- URL override

行为规则：

- 如果填写了 `URL override`，直接使用它
- 如果没有填写，Relora 会根据结构化字段自动拼接 URL
- `database` 字段对 server-level 连接不是必填

测试连接：

- 在 `Driver` 字段按 `t`
- 或在表单任意位置按 `Ctrl-T`

### 3. 浏览数据和结构

连接启动后：

1. 在左侧资源树里选择数据库、schema 和对象
2. 在 `Data` tab 看预览数据
3. 在 `Structure` tab 看字段结构
4. 在数据行上按 `Enter` 打开 row inspector

### 4. 执行 SQL

打开 SQL 编辑器的方式：

- `F3`
- `Ctrl-2`
- 或在浏览区按 `e`

进入之后可以：

- 编写 SQL
- 用 `F5` 或 `Ctrl-Enter` 执行当前 statement
- 切换 SQL tab 和结果集
- 用 `F10` 或 `Ctrl-R` 打开 SQL history 重跑历史 SQL
- 用 `F11` / `F12` 做 `EXPLAIN` 工作流

### 5. staged 编辑并提交

在数据表格中：

1. 移动到目标 cell
2. 按 `e`
3. 输入新值
4. 按 `Enter` 预览自动生成的 SQL
5. 在 SQL tab 中按 `Ctrl-G` 提交 staged transaction

## 常用快捷键

### 全局

- `Tab` / `Shift-Tab`：在 pane 之间切焦点
- `F2` / `Ctrl-1`：切到 `Data`
- `F3` / `Ctrl-2`：切到 `SQL`
- `F4` / `Ctrl-3`：切到 `Structure`
- `Ctrl-P`：命令面板
- `F10` / `Ctrl-R`：SQL history

### 浏览区

- `j` / `k` 或方向键上下：移动选择
- `Enter`、`Space`、`h`、`l`、左右方向键：展开 / 折叠
- `e`：打开 SQL 编辑器
- `s` / `i` / `u` / `x`：生成 CRUD 模板
- `r`：刷新
- `c`：取消任务

### 数据表格

- `j` / `k`：移动行
- `h` / `l`：移动列
- `PageUp` / `PageDown`：按页滚动
- `N` / `P`：下一页 / 上一页预览
- `y`：复制当前 row
- `Y`：复制当前 cell
- `w`：复制 `WHERE` 条件
- `e`：编辑当前 cell

### SQL 编辑器

- `F5` 或 `Ctrl-Enter`：执行当前 statement
- `F11`：`EXPLAIN`
- `F12`：`EXPLAIN ANALYZE`
- `Ctrl-T`：新建 SQL tab
- `Ctrl-W`：关闭 SQL tab
- `F6` / `F7`：切换 SQL tab
- `F8` / `F9`：切换结果集
- `Ctrl-G`：提交 staged CRUD

## Driver Sidecar 说明

Relora 不会在 TUI 里运行 `cargo install`。对终端用户来说，不应该要求本机先安装 Rust toolchain。

如果通过 npm 安装，sidecar 会跟随下载的 runtime bundle 一起落到本地，不需要单独安装。

当前 driver 查找顺序：

- `RELORA_POSTGRES_DRIVER` / `RELORA_MYSQL_DRIVER` / `RELORA_SQLITE_DRIVER`
- `PATH` 中的同名 binary
- Relora 主程序同目录下的同名 binary
- `~/.cargo/bin`
- workspace 的 `target/debug` 或 `target/release`

## CLI 参数

```bash
cargo run -p relora -- --help
```

- `--url`：单个数据库连接串
- `--connection`：命名连接，格式 `name=url`，可重复传入
- `--preview-limit`：预览行数上限，默认 `100`

## 给开发者的信息

### Monorepo 结构

```text
.
├── apps/
│   └── relora/
├── packages/
│   └── relora-npm/
├── scripts/
│   └── package-release-bundle.cjs
└── crates/
    ├── relora-app/
    ├── relora-core/
    ├── relora-driver-mysql/
    ├── relora-driver-postgres/
    └── relora-driver-sqlite/
```

### 各包职责

- `apps/relora`：可执行程序、CLI 配置、sidecar registry、TUI shell 与 `ratatui` 渲染
- `packages/relora-npm`：npm 安装器包，负责下载预编译 Relora bundle
- `scripts/package-release-bundle.cjs`：生成 npm / release 用版本化 bundle 的辅助脚本
- `crates/relora-app`：应用状态、workspace 投影、SQL 编辑器状态、CRUD 工具、只读 UI 视图
- `crates/relora-core`：共享数据库 trait 与领域模型
- `crates/relora-driver-postgres`：PostgreSQL sidecar
- `crates/relora-driver-mysql`：MySQL / MariaDB sidecar
- `crates/relora-driver-sqlite`：SQLite sidecar

### 验证命令

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### 构建 npm release bundle

在构建 release 二进制之后，可以用下面的脚本打出 npm 安装器所需的版本化 bundle：

```bash
cargo build --release -p relora -p relora-driver-postgres -p relora-driver-mysql -p relora-driver-sqlite
node scripts/package-release-bundle.cjs --platform darwin --arch arm64
```
