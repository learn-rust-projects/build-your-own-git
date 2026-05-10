# OwnGit - 从零实现 Git 核心功能

> Rust 实现版 Git，涵盖对象存储、打包协议、网络克隆等核心技术

---

## 简介

本项目是从零实现 Git 核心功能的 Rust 项目，完整实现了以下命令：

- `init` - 初始化 Git 仓库
- `hash-object` - 计算文件 SHA-1 哈希并可选写入对象库
- `cat-file` - 读取并展示 Git 对象内容
- `ls-tree` - 列出树对象条目
- `write-tree` - 将工作区目录结构写入树对象
- `commit-tree` / `commit` - 创建提交对象
- `clone` - 通过 Git 智能协议实现远程仓库克隆

---

## 技术栈

| 分类 | 技术选型 |
| ------ | ---------- |
| 语言 | Rust (Edition 2024) |
| 异步 | Tokio (full features) |
| HTTP 客户端 | Reqwest |
| 命令行解析 | Clap (derive 模式) |
| 压缩 | flate2 (zlib) |
| 哈希算法 | sha1 crate |
| 错误处理 | anyhow + thiserror |
| 临时文件 | tempfile |

---

## 架构设计

```text
src/
├── main.rs           # CLI 入口，子命令路由
├── commands.rs       # 命令模块导出
├── objects.rs        # Git 对象核心抽象
│                     # - Kind (Blob/Tree/Commit/Tag)
│                     # - Mode (文件权限)
│                     # - Object<T> 泛型对象
│                     # - HashWriter/HashReader (流式哈希计算)
│                     # - zlib 压缩/解压封装
│                     └── commands/
│                         ├── clone.rs       # Git 智能协议 + Packfile 解析
│                         ├── commit.rs      # 提交创建
│                         ├── hash_object.rs # 文件哈希计算
│                         ├── write_tree.rs  # 目录树构建
│                         ├── ls_tree.rs     # 树对象展示
│                         └── cat_file.rs    # 对象读取
```

---

## 关键技术难点 & 亮点 & 解决方案

### 1. Object泛型对象

**难点**：对象可以从本地文件、网络 packfile 等多种来源读取，需要支持任意实现了 `Read` 的数据源。

**解决方案**：

`Object<R>` 泛型对象需要支持任意实现了 `Read` 的数据源，同时需要维护对象类型 `Kind` 和预期大小 `u64`。

```rust
#[derive(Debug, Clone)]
pub(crate) struct Object<R> {
    pub(crate) kind: Kind,
    pub(crate) expected_size: u64,
    pub(crate) reader: R,
}
```

### 2. Packfile Delta 解析 (RefDelta / OfsDelta)

**难点**：Git packfile 使用 delta 压缩节省空间，支持引用 delta 和偏移 delta 两种形式，需解析变长编码的对象大小。

**解决方案**：

- 协议解析注重严格校验：使用bytes和精确读取语义化准确处理协议split_first_chunk、get(4..len)、read_exact、read_u8
- 实现MSB变长编码：实现 `read_size()` 函数处理 MSB 变长编码（每字节最高位表示是否继续）
- 实现解析RefDelta逻辑：`RefDelta` 维护 `HashMap<[u8; 20], Object>` 动态构建对象依赖图，解析 opcode 按位标志提取 copy offset/size 字节, 找到基本对象，以此为基础构造 RefDelta
- 流式解析避免一次性加载整个 packfile 到内存
- 使用 `join_all()` 并行处理多个对象的 delta 解析
- 高效计算哈希：使用包装器，一边读取一边计算哈希，最后校验哈希是否匹配

### 3. 使用包装器模式设计流式哈希计算和压缩场景 (HashWriter / HashReader)，支持泛型对象

**难点**：Git 对象存储需要**边写边算** SHA-1 哈希，且需区分压缩和非压缩场景。

**解决方案**：

- 设计 `HashWriter<W>` 装饰器模式，在 `write()` 时同步更新 `Sha1` 状态
- `Object::compute_hash()` 接收泛型 `Write`，同时支持 `ZlibEncoder`（压缩场景）和 `std::io::sink()`（仅计算哈希）
- 实现 `HashReader<R>` 支持 `Read` + `BufRead` + `finalize()` 返回 (hash, reader)

### 4. 条件编译适配跨平台文件模式检测

**难点**：Unix 可执行位与 Windows 扩展名判断逻辑不同。

**解决方案**：

- `Mode::from_meta()` 使用条件编译 `#[cfg(unix)]` / `#[cfg(windows)]`
- Unix 通过 `metadata.permissions().mode() & 0o111` 判断可执行位
- Windows 通过扩展名匹配 `exe/bat/cmd`

### 5. Tree 对象排序规则

**难点**：Git 要求 tree 条目按字节序排序（目录优先于同名文件），排序规则比较特殊，文档不够清晰。

**解决方案**：

- 查看官网源码然后实现
- 自定义比较器：对文件名字节进行比较，遇到相同前缀时以 `/` 判定目录优先
- 使用 `Path::as_encoded_bytes()` 获取平台无关的字节表示
