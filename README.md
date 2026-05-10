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

### 1. Packfile Delta 解析 (RefDelta / OfsDelta)

**难点**：Git packfile 使用 delta 压缩节省空间，支持引用 delta 和偏移 delta 两种形式，需解析变长编码的对象大小。

**解决方案**：

- 实现 `read_size()` 函数处理 MSB 变长编码（每字节最高位表示是否继续）
- `RefDelta` 维护 `HashMap<[u8; 20], Object>` 动态构建对象依赖图
- 解析 opcode 按位标志提取 copy offset/size 字节
- 流式解析避免一次性加载整个 packfile 到内存

### 2. 流式哈希计算 (HashWriter / HashReader)

**难点**：Git 对象存储需要**边写边算** SHA-1 哈希，且需区分压缩和非压缩场景。

**解决方案**：

- 设计 `HashWriter<W>` 装饰器模式，在 `write()` 时同步更新 `Sha1` 状态
- `Object::compute_hash()` 接收泛型 `Write`，同时支持 `ZlibEncoder`（压缩场景）和 `std::io::sink()`（仅计算哈希）
- 实现 `HashReader<R>` 支持 `Read` + `BufRead` + `finalize()` 返回 (hash, reader)

### 3. 跨平台文件模式检测

**难点**：Unix 可执行位与 Windows 扩展名判断逻辑不同。

**解决方案**：

- `Mode::from_meta()` 使用条件编译 `#[cfg(unix)]` / `#[cfg(windows)]`
- Unix 通过 `metadata.permissions().mode() & 0o111` 判断可执行位
- Windows 通过扩展名匹配 `exe/bat/cmd`

### 4. Git 智能协议网络交互

**难点**：需正确构造 `want`/`done` 请求，验证 pkt-line 格式，解析 symref。

**解决方案**：

- 构造符合协议的请求体（十六进制长度前缀）
- 使用正则 `^[0-9a-f]{4}#` 验证服务端响应前缀
- `parse_pkt_lines()` 循环解析变长 pkt-line 流
- 正则提取 `symref=HEAD:refs/heads/<branch>` 获取分支名

### 5. 异步文件 I/O

**难点**：所有文件操作均为异步，与标准库的同步 `Write`/`Read` 需要桥接。

**解决方案**：

- Tokio `fs` 模块处理目录遍历和文件写入
- `write_object()` 使用 `NamedTempFile` 创建临时文件，原子性 `rename` 到最终路径
- 异步上下文中的哈希计算需 `.await`，保持非阻塞

### 6. Tree 对象排序规则

**难点**：Git 要求 tree 条目按字节序排序（目录优先于同名文件）。

**解决方案**：

- 自定义比较器：对文件名字节进行比较，遇到相同前缀时以 `/` 判定目录优先
- 使用 `Path::as_encoded_bytes()` 获取平台无关的字节表示

---

## 设计亮点

1. **泛型抽象**：`Object<R>` 支持任意实现了 `Read` 的数据源，可复用解析本地对象和网络 packfile
2. **装饰器模式**：`MaybeCompress` 运行时决定是否压缩，`HashWriter`/`HashReader` 透明添加哈希计算
3. **零拷贝设计**：packfile 解析后直接存入 `HashMap`，避免不必要的内存复制
4. **原子性写入**：使用 `NamedTempFile` + `rename` 确保写入过程不会产生损坏的对象文件
5. **async/await 完整链路**：从 HTTP 请求到文件落盘全程异步，充分利用 Tokio 生态
