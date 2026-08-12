#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    ffi::CStr,
    fmt,
    fs::Metadata,
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Error};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use tempfile::NamedTempFile;
use tokio::fs;

use crate::commands::clone::ObjectType;
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum Kind {
    Blob,
    Tree,
    Commit,
    Tag,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Blob => write!(f, "blob"),
            Kind::Tree => write!(f, "tree"),
            Kind::Commit => write!(f, "commit"),
            Kind::Tag => write!(f, "tag"),
        }
    }
}

impl From<Mode> for Kind {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::File => Kind::Blob,
            Mode::Executable => Kind::Blob,
            Mode::Directory => Kind::Tree,
            Mode::SymbolicLink => Kind::Tag,
        }
    }
}
impl From<ObjectType> for Kind {
    fn from(obj_type: ObjectType) -> Self {
        match obj_type {
            ObjectType::Commit => Kind::Commit,
            ObjectType::Tree => Kind::Tree,
            ObjectType::Blob => Kind::Blob,
            ObjectType::Tag => Kind::Tag,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    File,
    Executable,
    Directory,
    SymbolicLink,
}

impl Mode {
    pub fn from_str(s: &str) -> anyhow::Result<Mode, Error> {
        match s {
            "40000" => Ok(Mode::Directory),
            "040000" => Ok(Mode::Directory),
            "100644" => Ok(Mode::File),
            "100755" => Ok(Mode::Executable),
            "120000" => Ok(Mode::SymbolicLink),
            _ => anyhow::bail!("unknown kind: {s}"),
        }
    }
    pub fn is_dir(&self) -> bool {
        matches!(self, Mode::Directory)
    }
    pub fn to_bytes(&self) -> &'static [u8] {
        match self {
            Mode::File => b"100644",
            Mode::Executable => b"100755",
            Mode::Directory => b"40000",
            Mode::SymbolicLink => b"120000",
        }
    }
    /// 从文件元数据判断 Mode
    pub fn from_meta(metadata: &Metadata) -> Mode {
        let ft = metadata.file_type();

        if ft.is_dir() {
            Mode::Directory
        } else if ft.is_symlink() {
            Mode::SymbolicLink
        } else if ft.is_file() {
            #[cfg(unix)]
            {
                let mode = metadata.permissions().mode();
                if mode & 0o111 != 0 {
                    return Mode::Executable;
                }
            }

            #[cfg(windows)]
            {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext_lower = ext.to_ascii_lowercase();
                    if matches!(ext_lower.as_str(), "exe" | "bat" | "cmd") {
                        return Mode::Executable;
                    }
                }
            }

            Mode::File
        } else {
            // fallback
            Mode::File
        }
    }

    /// 从路径直接判断 Mode
    pub async fn from_path(path: &Path) -> std::io::Result<Mode> {
        let metadata = fs::symlink_metadata(path).await?; // 保留符号链接
        Ok(Self::from_meta(&metadata))
    }
}

struct HashWriter<W> {
    writer: W,
    hasher: Sha1,
}

impl<W> Write for HashWriter<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.writer.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

pub struct HashReader<R> {
    inner: R,
    hasher: Sha1,
}
impl<R: BufRead> HashReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha1::new(),
        }
    }

    pub fn finalize(self) -> ([u8; 20], R) {
        (self.hasher.finalize().into(), self.inner)
    }
}
impl<R: Read> Read for HashReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
}
impl<R: BufRead> BufRead for HashReader<R>
where
    R: BufRead,
{
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        let buf = &self.inner.fill_buf().unwrap()[..amt];
        self.hasher.update(buf); // ✅ 在 consume 时更新哈希
        self.inner.consume(amt);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Object<R> {
    pub(crate) kind: Kind,
    pub(crate) expected_size: u64,
    pub(crate) reader: R,
}

pub(crate) async fn hash_to_reader(path: &str) -> anyhow::Result<Object<impl BufRead>> {
    // TODO
    // 使用string构造路径
    let f = std::fs::File::open(format!(".git/objects/{}/{}", &path[0..2], &path[2..]))
        .context("open in .git/objects")?;
    let decoder = ZlibDecoder::new(f);
    let mut buf = std::io::BufReader::new(decoder);

    let mut ret = Vec::new();
    // 1. 读取文件头
    buf.read_until(b'\0', &mut ret)?;
    // let s = std::str::from_utf8(&ret).unwrap();
    let c_str = CStr::from_bytes_with_nul(&ret).expect("Invalid C string");
    let header = c_str
        .to_str()
        .context(" .git/objects file header isn't valid utf-8")?;
    // 使用split_once 而不是split 是为了避免文件名中包含空格
    let Some((kind, size)) = header.split_once(' ') else {
        anyhow::bail!(".git/objects file header did not start with a konw type {header}");
    };
    // 处理类型
    let kind = match kind {
        "blob" => Kind::Blob,
        "tree" => Kind::Tree,
        "commit" => Kind::Commit,
        "tag" => Kind::Tag,
        _ => anyhow::bail!("we do not know how to print a '{kind}'"),
    };

    // 要得到 usize，必须显式解析：
    let size = size
        .parse::<u64>()
        .context(" .git/objects file header size isn't valid:{size}")?;
    let buf = buf.take(size);
    Ok(Object {
        kind,
        expected_size: size,
        reader: buf,
    })
}

/// 包装器，根据 compress 决定是否压缩
enum MaybeCompress<W: Write> {
    Compressed(ZlibEncoder<W>),
    Plain(W),
}

impl<W: Write> Write for MaybeCompress<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            MaybeCompress::Compressed(z) => z.write(buf),
            MaybeCompress::Plain(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            MaybeCompress::Compressed(z) => z.flush(),
            MaybeCompress::Plain(w) => w.flush(),
        }
    }
}

impl<W: Write> MaybeCompress<W> {
    fn finish(&mut self) -> std::io::Result<()> {
        match self {
            MaybeCompress::Compressed(z) => z.try_finish(), // 完成压缩
            MaybeCompress::Plain(_) => Ok(()),
        }
    }
}

impl<R> Object<R>
where
    R: Read,
{
    /// 计算hash和压缩：计算hash一定执行，根据 compress 决定是否压缩
    pub(crate) async fn compute_hash(
        &mut self,
        writer: impl Write,
        compress: bool,
    ) -> Result<[u8; 20], anyhow::Error> {
        // 1、根据compress是否要压缩，包装writer
        let writer = if compress {
            MaybeCompress::Compressed(ZlibEncoder::new(writer, Compression::default()))
        } else {
            MaybeCompress::Plain(writer)
        };

        // 2、使用HashWriter 包装writer，HashWriter 会计算写入的内容的hash
        let mut writer = HashWriter {
            writer,
            hasher: Sha1::new(),
        };
        write!(writer, "{} {}\0", self.kind, self.expected_size)?;
        // 3、将reader 中的内容写入writer
        std::io::copy(&mut self.reader, &mut writer).context("stream file into blob")?;

        // 4. 计算hash和压缩，hash是和压缩一起进行的
        writer.writer.finish()?;
        let sha1 = writer.hasher.finalize();
        Ok(sha1.into())
    }

    /// 将压缩后的对象写入 .git/objects 目录
    pub(crate) async fn write_object(&mut self, path: PathBuf) -> Result<[u8; 20], anyhow::Error> {
        // 1、使用tempfile crate创建临时文件
        let tmp_path = NamedTempFile::new()?.into_temp_path();
        let file: std::fs::File = std::fs::File::create(&tmp_path)?;

        // 2、计算hash 压缩写入临时文件
        let hex_sha1 = self
            .compute_hash(file, true)
            .await
            .context("compute hash failed")?;
        let hex = hex::encode(hex_sha1);

        // 3、重命名文件，将临时文件重命名为最终的文件
        // TODO: 优化：重复了就直接舍弃这个临时文件，不去替换了
        // 如果 new 不存在 → 直接移动
        // 如果 new 存在 → 原子替换
        fs::create_dir_all(path.join(format!(".git/objects/{}/", &hex[..2]))).await?;
        std::fs::rename(
            tmp_path,
            path.join(format!(".git/objects/{}/{}", &hex[..2], &hex[2..])),
        )
        .context("move blob file into .git/objects")?;

        Ok(hex_sha1)
    }
}
/// 将文件转换为 Object 类型
pub(crate) fn file_to_object(file: impl AsRef<Path>) -> anyhow::Result<Object<impl Read>> {
    let file = file.as_ref();
    let stat = std::fs::metadata(file).with_context(|| format!("stat {}", file.display()))?;
    // TODO: technically there's a race here if the file changes between stat and
    // write
    let file = std::fs::File::open(file).with_context(|| format!("open {}", file.display()))?;
    Ok(Object {
        kind: Kind::Blob,
        expected_size: stat.len(),
        reader: file,
    })
}

pub(crate) async fn git_init(dir: PathBuf, mut branch: &str) -> anyhow::Result<()> {
    const GIT_DIR: &str = ".git";

    // 创建目录列表
    let dirs = ["objects", "refs"];
    fs::create_dir(dir.join(GIT_DIR))
        .await
        .context("create git dir fail")?;
    for d in dirs {
        fs::create_dir(dir.join(GIT_DIR).join(d))
            .await
            .with_context(|| format!("create git {d} dir fail"))?;
    }
    if branch.is_empty() {
        branch = "refs/heads/main";
    }

    // 创建 HEAD 文件
    fs::write(dir.join(GIT_DIR).join("HEAD"), format!("ref: {branch}\n"))
        .await
        .context("create git HEAD fail")?;

    Ok(())
}

pub async fn write_ref_file(path: PathBuf, data: &[u8]) -> anyhow::Result<()> {
    // 1. 创建父目录
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // 2. 写入文件
    fs::write(path, hex::encode(data)).await?;

    Ok(())
}

pub(crate) fn find_headref(path: PathBuf) -> anyhow::Result<String> {
    let head_ref = std::fs::read_to_string(path.join(".git/HEAD"))?;
    let Some(head_ref) = head_ref.strip_prefix("ref: ") else {
        anyhow::bail!("refusing to commit onto detached HEAD");
    };
    // 去除末尾的换行符
    let head_ref = head_ref.trim_end();
    Ok(head_ref.to_string())
}
