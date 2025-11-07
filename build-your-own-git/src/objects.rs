#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    ffi::CStr,
    fmt,
    fs::Metadata,
    io::{BufRead, Read, Write},
    path::Path,
};

use anyhow::{Context, Error};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use tempfile::NamedTempFile;
use tokio::fs;

#[derive(Debug, PartialEq, Eq)]
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
            Mode::Directory => b"040000",
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

#[derive(Debug)]
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
        let _ = writer.writer.finish()?;
        let sha1 = writer.hasher.finalize();
        Ok(sha1.into())
    }

    /// 将压缩后的对象写入 .git/objects 目录
    pub(crate) async fn write_object(&mut self) -> Result<[u8; 20], anyhow::Error> {
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
        fs::create_dir_all(format!(".git/objects/{}/", &hex[..2])).await?;
        std::fs::rename(
            tmp_path,
            format!(".git/objects/{}/{}", &hex[..2], &hex[2..]),
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
