# OwnGit - 从零实现 Git 核心功能

> Rust 实现版 Git，涵盖对象存储、Packfile 协议解析、Git 智能协议等核心技术

---

## 项目概述

用 Rust 从零实现 Git 核心功能，深入理解版本控制系统底层原理。

### 已实现命令

| 命令   | 功能                              |
|--------|-----------------------------------|
| `init` | 初始化 Git 仓库                   |
| `hash-object` | 计算文件 SHA-1 哈希并可选写入对象库 |
| `cat-file` | 读取并展示 Git 对象内容          |
| `ls-tree` | 列出树对象条目                   |
| `write-tree` | 将工作区目录结构写入树对象       |
| `commit` / `commit-tree` | 创建提交对象           |
| `clone` | 通过 Git 智能协议实现远程仓库克隆 |

---

## 技术栈

| 分类       | 技术选型            |
|------------|--------------------|
| 语言       | Rust (Edition 2024) |
| 异步       | Tokio (full features) |
| HTTP 客户端 | Reqwest            |
| 命令行解析  | Clap (derive 模式) |
| 压缩       | flate2 (zlib)      |
| 哈希算法   | sha1 crate         |
| 错误处理   | anyhow + thiserror |
| 临时文件   | tempfile           |

---

## 架构设计

```
src/
├── main.rs           # CLI 入口，子命令路由
├── commands.rs       # 命令模块导出
├── objects.rs        # Git 对象核心抽象
│                     # - Kind (Blob/Tree/Commit/Tag)
│                     # - Mode (文件权限，跨平台)
│                     # - Object<T> 泛型对象
│                     # - HashWriter/HashReader (流式哈希计算)
│                     └── commands/
│                         ├── clone.rs       # Git 智能协议 + Packfile 解析
│                         ├── commit.rs      # 提交创建
│                         ├── hash_object.rs # 文件哈希计算
│                         ├── write_tree.rs  # 目录树构建
│                         ├── ls_tree.rs     # 树对象展示
│                         └── cat_file.rs    # 对象读取
```

---

## 核心技术亮点

### 底层协议实现

实现 Git 底层通信协议，深入理解分布式版本控制原理：

- **Git Packfile 协议**：packfile 格式解析、PACK header 校验、对象计数解析
- **Git 智能协议**：pkt-line 格式解析、refs 发现、symref 解析、NAK/ACK 握手机制
- **MSB 变长编码**：Git 协议中用于表示对象大小的可变长编码实现
- **Delta 压缩解析**：RefDelta 和 OfsDelta 的 opcode 按位解析

### Git 对象抽象

设计泛型 `Object<R>` 对象抽象，支持任意实现了 `Read` 的数据源：

- 本地 `.git/objects` 文件读取
- 网络 packfile 流式读取
- 统一的接口设计，零成本抽象

### 接口设计

基于 Rust 高级特性设计零成本抽象 API：

- **泛型**：`Object<R>` 支持任意 reader 类型
- **impl Trait**：函数返回值使用 `impl Trait` 简化接口
- **trait object**：`Pin<Box<dyn Future>>` 实现异步递归
- **AsRef 模式**：`file_to_object()` 接受任意可转为路径的类型

### 设计模式

使用装饰器模式实现核心功能：

- **HashWriter**：包装 `Write`，边写边算 SHA-1 哈希
- **HashReader**：包装 `Read`，边读边算哈希，支持 `finalize()` 返回 (hash, reader)
- **MaybeCompress**：根据参数决定是否压缩的策略模式

### 性能优化

充分利用 Rust 异步运行时和网络 I/O：

- **流式解析**：使用 `BufReader` 流式解析 packfile，逐对象处理后即释放内存，避免大文件内存爆炸
- **并行写入**：使用 Tokio `JoinSet` 并行写入对象文件，充分利用异步 I/O
- **原子写入**：使用 tempfile + rename 确保写入失败时不影响原有数据

### AI 提效

使用 AI 辅助开发：

- 完整的集成测试编写
- 详细的代码文档和注释

---

## 协议

- Apache License 2.0
- MIT License
