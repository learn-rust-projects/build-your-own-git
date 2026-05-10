use std::{
    any,
    collections::HashMap,
    f32::consts::E,
    ffi::CStr,
    fs,
    io::{BufRead, Cursor, Read, Write},
    path::{Path, PathBuf},
    ptr::read,
    sync::LazyLock,
};

use anyhow::{Context, Result, bail};
use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Bytes, buf};
use clap::builder::Str;
use flate2::{Compression, bufread::ZlibDecoder, write::ZlibEncoder};
use regex::Regex;
use reqwest::StatusCode;
use tokio_util::io::StreamReader;

use crate::objects::{HashReader, Kind, Mode, Object};

// Regex::new(r"^[0-9a-f]{4}#") 是 运行时函数，会解析正则并构建 DFA
static RESPONSE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{4}#").unwrap());

#[repr(u8)]
#[derive(Debug)]
pub enum ObjectType {
    Commit = 1,
    Tree = 2,
    Blob = 3,
    Tag = 4,
    OfsDelta = 6,
    RefDelta = 7,
}
impl ObjectType {
    fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(ObjectType::Commit),
            2 => Some(ObjectType::Tree),
            3 => Some(ObjectType::Blob),
            4 => Some(ObjectType::Tag),
            6 => Some(ObjectType::OfsDelta),
            7 => Some(ObjectType::RefDelta),
            _ => None, // 避免无效 u8
        }
    }
}

pub(crate) async fn invoke(repo_url: String, path: PathBuf) -> Result<(), anyhow::Error> {
    // 1. 构造和发送 git-upload-pack 请求
    let repo_url = repo_url.trim_end_matches(".git").trim_end_matches('/');
    let client = reqwest::Client::new();
    let git_url = format!("{}/info/refs?service=git-upload-pack", repo_url);
    let resp = client.get(&git_url).send().await?;
    // 1.1 客户必须验证状态码是否为 200 OK 或 200 错误。
    let bytes = validate_status_and_return_body(resp, &git_url).await?;

    // 2. 客户端必须验证响应实体的前五个字节是否与正则表达式 ^ [ 0-9a-f ] {4}#
    // 匹配。如果此测试失败，客户端不得继续。
    validate_response(&bytes)?;

    // 3. 客户端必须将整个响应解析为一系列 pkt-line 记录。
    let pkt_lines = parse_pkt_lines(&bytes)?;
    let line = &pkt_lines[1];

    // 4. 解析 symref 和 hash
    // 4.1 解析前缀：hash + 余下内容
    let (hash, rest) = line
        .split_once(' ')
        .context("failed to split pkt-line into <hash> <capabilities>")?;
    eprintln!("first hash: {}", hash);

    // 4.2 找 symref并提取分支名
    let symref_prefix = "symref=HEAD:";
    let branch = rest
        .split_once(symref_prefix)
        .ok_or_else(|| anyhow::anyhow!("missing `symref=HEAD:` capability"))
        .and_then(|(_, r)| {
            r.split_whitespace()
                .next()
                .ok_or_else(|| anyhow::anyhow!("missing branch after `symref=HEAD:`"))
        })?;
    eprintln!("branch: {}", branch);

    // 5. 请求packfile
    let clone_url = format!("{}/git-upload-pack", repo_url);

    let mut req_body = Vec::new();
    writeln!(req_body, "0032want {}", hash)?;
    writeln!(req_body, "00000009done")?;

    let commit_hash = hex::decode(hash)?;
    let resp = client
        .post(&clone_url)
        .header("Content-Type", "application/x-git-upload-pack-request")
        .body(req_body)
        .send()
        .await?;
    let body = Cursor::new(validate_status_and_return_body(resp, &clone_url).await?);
    // &[u8] 本身实现了 Read：
    let mut bufreader = std::io::BufReader::new(body);
    // 5.1 解析packfile header
    let mut nak_vec = vec![0; 8];
    bufreader
        .read_exact(&mut nak_vec)
        .context("read nak fail")?;

    if nak_vec != b"0008NAK\n" {
        anyhow::bail!("expected NAK, got {:?}", String::from_utf8_lossy(&nak_vec));
    }
    let mut bufreader = HashReader::new(bufreader);
    let mut pack = vec![0; 4];
    bufreader.read_exact(&mut pack).context("read pack fail")?;
    if pack != b"PACK" {
        anyhow::bail!("expected PACK, got {:?}", String::from_utf8_lossy(&pack));
    }

    // 版本号，接下来四字节
    let packfile_version = bufreader
        .read_u32::<BigEndian>()
        .context("read packfile version fail")?;
    eprintln!("packfile_version: {}", packfile_version);

    // 打包文件数量，接下来四字节
    let num_objects = bufreader
        .read_u32::<BigEndian>()
        .context("read packfile object count fail")?;
    eprintln!("num_objects: {}", num_objects);
    let mut hashmap: HashMap<[u8; 20], Object<Cursor<Vec<u8>>>> = std::collections::HashMap::new();
    // 5.2 解析packfile objects
    for object_index in 0..num_objects {
        let (size, obj_type) = read_size(&mut bufreader, false).context("read object size fail")?;
        if let Some(obj_type) = ObjectType::from_u8(obj_type) {
            match obj_type {
                ObjectType::Commit | ObjectType::Tree | ObjectType::Blob | ObjectType::Tag => {
                    eprintln!("object is of type {:?}", obj_type);
                    let mut object = Object {
                        kind: Kind::from(obj_type),
                        expected_size: size as u64,
                        reader: read_one_object(&mut bufreader)?,
                    };
                    let hash = object.compute_hash(std::io::sink(), false).await?;
                    object.reader.set_position(0);
                    eprintln!(
                        "Object {} is of type {:?} with hash {:?}",
                        object_index,
                        object.kind,
                        hex::encode(hash)
                    );
                    hashmap.insert(hash, object);
                }
                ObjectType::RefDelta => {
                    // 读取 base object id
                    let mut base_object_id = [0; 20];
                    bufreader
                        .read_exact(&mut base_object_id)
                        .context("read base object id fail")?;
                    eprintln!("base_object_id: {:?}", hex::encode(base_object_id));

                    let hex_base_object_id = base_object_id;
                    let base_object = hashmap
                        .get(&hex_base_object_id)
                        .context("can not find base object")?;

                    let mut delta_data = read_one_object(&mut bufreader)?;
                    let (src_size, _) =
                        read_size(&mut delta_data, true).context("read src size fail")?;
                    let (tgt_size, _) =
                        read_size(&mut delta_data, true).context("read tgt size fail")?;
                    eprintln!("src_size: {}, tgt_size: {}", src_size, tgt_size);
                    let mut new_tgt = Vec::with_capacity(tgt_size);
                    while delta_data.position() < delta_data.get_ref().len() as u64 {
                        let opcode = delta_data.read_u8().context("read delta opcode fail")?;
                        if (opcode & 0x80) != 0 {
                            // copy from base
                            let mut copy_offset = 0;
                            let mut copy_size = 0;
                            if (opcode & 0b0000_0001) != 0 {
                                let b = delta_data
                                    .read_u8()
                                    .context("read delta copy offset byte fail")?;
                                copy_offset |= b as usize;
                            }
                            if (opcode & 0b0000_0010) != 0 {
                                let b = delta_data
                                    .read_u8()
                                    .context("read delta copy offset byte fail")?;
                                copy_offset |= (b as usize) << 8;
                            }
                            if (opcode & 0b0000_0100) != 0 {
                                let b = delta_data
                                    .read_u8()
                                    .context("read delta copy offset byte fail")?;
                                copy_offset |= (b as usize) << 16;
                            }
                            if (opcode & 0b0000_1000) != 0 {
                                let b = delta_data
                                    .read_u8()
                                    .context("read delta copy offset byte fail")?;
                                copy_offset |= (b as usize) << 24;
                            }

                            if (opcode & 0b0001_0000) != 0 {
                                let b = delta_data
                                    .read_u8()
                                    .context("read delta copy size byte fail")?;
                                copy_size |= b as usize;
                            }
                            if (opcode & 0b0010_0000) != 0 {
                                let b = delta_data
                                    .read_u8()
                                    .context("read delta copy size byte fail")?;
                                copy_size |= (b as usize) << 8;
                            }
                            if (opcode & 0b0100_0000) != 0 {
                                let b = delta_data
                                    .read_u8()
                                    .context("read delta copy size byte fail")?;
                                copy_size |= (b as usize) << 16;
                            }
                            if copy_size == 0 {
                                copy_size = 0x1000;
                            }

                            let copy_end = copy_offset + copy_size;
                            if copy_end > src_size {
                                anyhow::bail!("copy end out of src size");
                            }

                            new_tgt.extend_from_slice(
                                &base_object.reader.get_ref()[copy_offset..copy_end],
                            ); // no-op, just for clarity
                        } else {
                            let size = (opcode & 0b0111_1111) as usize;
                            new_tgt.extend_from_slice(
                                &delta_data.get_ref()[delta_data.position() as usize
                                    ..delta_data.position() as usize + size],
                            );
                            delta_data.set_position(delta_data.position() as u64 + size as u64);
                        }
                    }

                    let mut object = Object {
                        kind: base_object.kind.clone(),
                        expected_size: tgt_size as u64,
                        reader: Cursor::new(new_tgt),
                    };
                    let hash = object.compute_hash(std::io::sink(), false).await?;
                    object.reader.set_position(0);
                    eprintln!(
                        "RefDelta Object {} is of type {:?} with hash {:?}",
                        object_index,
                        object.kind,
                        hex::encode(hash)
                    );
                    hashmap.insert(hash, object);
                }
                ObjectType::OfsDelta => {
                    eprintln!("OfsDelta")
                }
            }
        } else {
            eprintln!("Unknown Object Type: {:?}", obj_type);
        }

        // println!("object1: {}", String::from_utf8_lossy(&object1));
        // break;
    }
    // 5.3 解析packfile footer
    let (hash, mut bufreader) = bufreader.finalize();
    eprintln!("final packfile hash: {}", hex::encode(hash));

    let mut expect_hash = Vec::new();
    bufreader
        .read_to_end(&mut expect_hash)
        .context("read final hash fail")?;
    eprintln!("expected packfile hash: {}", hex::encode(&expect_hash));
    anyhow::ensure!(
        hex::encode(hash) == hex::encode(&expect_hash),
        "packfile hash mismatch: expected {}, got {}",
        hex::encode(&expect_hash),
        hex::encode(hash)
    );

    let tree_hash = parse_tree_hash(&commit_hash, &hashmap)?;
    tree_to_file(path.to_path_buf(), &tree_hash, &hashmap)?;
    crate::objects::git_init(path.to_path_buf(), branch).await?;
    let head_ref = crate::objects::find_headref(path.to_path_buf())?;
    eprintln!("head ref: {}", head_ref);
    crate::objects::write_ref_file(path.join(format!(".git/{head_ref}")), &commit_hash)
        .await
        .context("write ref file fail")?;
    // 写入object文件

    let mut set = tokio::task::JoinSet::new();

    for (_, mut object) in hashmap {
    let path = path.to_path_buf();
    set.spawn(async move {
        object.write_object(path).await
    });

    while let Some(res) = set.join_next().await {
        res??;
    }
}
    Ok(())
}

fn parse_tree_hash(
    commit_hash: &[u8],
    hashmap: &HashMap<[u8; 20], Object<Cursor<Vec<u8>>>>,
) -> anyhow::Result<[u8; 20]> {
    let mut head = hashmap
        .get(commit_hash)
        .context("can not find head object")?
        .clone();
    head.reader.set_position(5);
    let mut hash = [0; 40];
    head.reader.read_exact(&mut hash)?;
    let tree_hash = hex::decode(hash)?;
    tree_hash
        .try_into()
        .map_err(|v: Vec<u8>| anyhow::anyhow!("hash length invalid: {:?}", v))
}
fn tree_to_file(
    path: PathBuf,
    tree_hash: &[u8],
    hashmap: &HashMap<[u8; 20], Object<Cursor<Vec<u8>>>>,
) -> anyhow::Result<()> {
    fs::create_dir_all(&path).context("create dir all fail")?;
    let mut hash_object = hashmap
        .get(tree_hash)
        .context("can not find tree object")?
        .clone();
    let mut buf = Vec::new();
    let mut hashbuf = [0; 20];
    loop {
        let n = hash_object
            .reader
            .read_until(0, &mut buf)
            .context("read next tree object entry")?;
        if n == 0 {
            break;
        }
        eprintln!("{:?}", String::from_utf8_lossy(&buf));
        let mode_and_name = CStr::from_bytes_with_nul(&buf)
            .context("invalid tree entry")?
            .to_str()
            .context("invalid tree entry")?;
        // split_once https://github.com/rust-lang/rust/issues/112811
        // mode 权限设置，非核心，暂时忽略
        let (mode, name) = mode_and_name
            .split_once(' ')
            .context("split always yields once")?;

        hash_object
            .reader
            .read_exact(&mut hashbuf)
            .context("read entry hash fail")?;
        let kind: Kind = Mode::from_str(mode)?.into();
        match kind {
            Kind::Tree => {
                tree_to_file(path.join(name), &hashbuf, hashmap)?;
            }
            Kind::Blob => {
                eprintln!("blob hash: {}", hex::encode(hashbuf));
                let blob_path = path.join(name);
                let content = &hashmap
                    .get(hashbuf.as_slice())
                    .context("can not find blob object")?
                    .reader;
                fs::write(blob_path, content.get_ref())?; // 自动创建文件并写入
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn read_size<R: BufRead>(reader: &mut R, is_delta: bool) -> anyhow::Result<(usize, u8)> {
    let mut byte = reader.read_u8().context("read object type fail")?;
    let obj_type = (byte >> 4) & 0b111;
    let mut size = if is_delta {
        byte & 0b0111_1111
    } else {
        byte & 0b0000_1111
    } as usize;
    let mut shift = if is_delta { 7 } else { 4 };
    // 继续读取后续字节，直到最高位为 0
    while (byte & 0b1000_0000) != 0 {
        byte = reader.read_u8().context("read object size fail")?;
        size |= ((byte & 0b0111_1111) as usize) << shift;
        shift += 7;
    }
    Ok((size, obj_type))
}
/// 校验 Git Upload-Pack 的 HTTP 状态码
pub async fn validate_status_and_return_body(
    resp: reqwest::Response,
    url: &str,
) -> Result<Bytes, anyhow::Error> {
    let status: StatusCode = resp.status();
    eprintln!("url: {}, status: {}", url, status);
    match status {
        StatusCode::OK => Ok(resp.bytes().await?),
        StatusCode::NOT_FOUND => {
            bail!("repository not found");
        }

        _ => {
            bail!("clone failed");
        }
    }
}

fn validate_response(body: &[u8]) -> Result<(), anyhow::Error> {
    // 读取前五个字节并转为字符串
    let first_five_str = str::from_utf8(
        body.get(..5)
            .ok_or_else(|| anyhow::anyhow!("Response too short"))?,
    )?;
    anyhow::ensure!(
        RESPONSE_RE.is_match(first_five_str),
        "Response validation failed"
    );
    Ok(())
}
fn read_one_object<R: BufRead>(reader: &mut R) -> anyhow::Result<Cursor<Vec<u8>>> {
    // 不需要 BufReader 包 ZlibDecoder，直接解码即可
    let mut decoder = ZlibDecoder::new(reader);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;
    Ok(Cursor::new(buf))
}

fn parse_pkt_lines(mut body: &[u8]) -> Result<Vec<String>, anyhow::Error> {
    let mut pkt_lines = Vec::new();

    while !body.is_empty() {
        // 1.读取前 4 个字节作为长度
        let (len_bytes, rest) = body.split_first_chunk::<4>().ok_or_else(|| {
            anyhow::anyhow!(
                "Incomplete pkt-line header: expected 4 bytes for length, got {}",
                body.len()
            )
        })?;

        // 2. 解析十六进制长度（Git pkt-line 使用十六进制，不是十进制！）
        let len_str =
            str::from_utf8(len_bytes).context("Invalid UTF-8 in pkt-line length header")?;

        let len = usize::from_str_radix(len_str, 16)
            .with_context(|| format!("Invalid hexadecimal pkt-line length: '{}'", len_str))?;

        eprintln!("pkt-line length: {}", len);
        // 3. 处理 flush packet (长度为0)
        if len == 0 {
            body = rest;
            continue;
        }

        // 4. 严格的长度校验
        anyhow::ensure!(
            len >= 4,
            "Invalid pkt-line length: {} (minimum valid length is 4)",
            len
        );
        anyhow::ensure!(
            body.len() >= len,
            "Insufficient data for pkt-line: need {} bytes, have {}",
            len,
            body.len()
        );
        // 5. 解析内容为 UTF-8 字符串
        // 提取 pkt-line 内容（不包含长度字段）
        let content_str = str::from_utf8(
            body.get(4..len)
                .ok_or_else(|| anyhow::anyhow!("Invalid pkt-line content"))?,
        )
        .context("Invalid UTF-8 in pkt-line content")?;
        pkt_lines.push(content_str.to_string());
        eprint!("pkt-line content: {}", content_str);
        body = &body[len..];
    }

    Ok(pkt_lines)
}

// test clone
#[cfg(test)]

mod tests {
    use super::*;
    #[tokio::test]
    async fn test_clone() {
        let repo_url = "https://github.com/learn-rust-projects/build-your-own-git.git";
        let result = invoke(repo_url.to_string(), PathBuf::from("test-repo")).await;
        if let Err(e) = &result {
            eprintln!("clone error: {:?}", e);
        }
        assert!(result.is_ok());
    }
}
