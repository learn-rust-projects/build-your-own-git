use std::{
    any,
    f32::consts::E,
    io::{BufRead, Cursor, Read, Write},
    os::linux::raw::stat,
    ptr::read,
};

use anyhow::{Context, Result, bail};
use byteorder::{BigEndian, ReadBytesExt};
use flate2::{Compression, bufread::ZlibDecoder, write::ZlibEncoder};
use regex::Regex;
use reqwest::StatusCode;
use tokio_util::io::StreamReader;

use crate::objects::{Kind, Object};

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

pub(crate) async fn invoke(repo_url: String) -> Result<(), anyhow::Error> {
    let repo_url = repo_url.trim_end_matches(".git").trim_end_matches('/');
    let client = reqwest::Client::new();
    // let git_url = format!("{}/info/refs?service=git-upload-pack", repo_url);
    // let mut resp = client.get(&git_url).send().unwrap();
    // // 1. 客户必须验证状态码是否为 200 OK 或 200 错误。
    // let vec = validate_status_and_return_body(&mut resp, &git_url)?;

    // // 2. 客户端必须验证响应实体的前五个字节是否与正则表达式 ^ [ 0-9a-f ] {4}#
    // // 匹配。如果此测试失败，客户端不得继续。
    // validate_response(&vec)?;

    // // 3. 客户端必须将整个响应解析为一系列 pkt-line 记录。
    // let pkt_lines = parse_pkt_lines(&vec)?;
    // let (hash, _) = &pkt_lines[1]
    //     .split_once(' ')
    //     .context("split always yields once")?;
    // eprintln!("first hash: {}", hash);

    // TODO测试 之后需要删除
    // let hash = "9671f5a72cac8b4c379b1c35a6af6d10611d620f";

    // let clone_url = format!("{}/git-upload-pack", repo_url);

    // let mut req_body = Vec::new();
    // writeln!(req_body, "0032want {}", hash)?;
    // writeln!(req_body, "00000009done")?;

    // let mut resp = client
    //     .post(&clone_url)
    //     .header("Content-Type", "application/x-git-upload-pack-request")
    //     .body(req_body)
    //     .send()
    //     .unwrap();
    // let body = validate_status_and_return_body(&mut resp, &clone_url)?;
    // &[u8] 本身实现了 Read：
    // 写入到一个文件里面
    // TODO 之后需要删除 写入文件中
    // let mut file = std::fs::File::create("../packfile_code.bin")?;
    // std::io::copy(&mut body.as_slice(), &mut file)?;
    let mut file = std::fs::File::open("../packfile.bin")?;
    let mut bufreader = std::io::BufReader::new(&mut file);

    let mut nak_vec = vec![0; 8];
    bufreader
        .read_exact(&mut nak_vec)
        .context("read nak fail")?;

    if nak_vec != b"0008NAK\n" {
        anyhow::bail!("expected NAK, got {:?}", String::from_utf8_lossy(&nak_vec));
    }

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
    let mut hashmap = std::collections::HashMap::new();
    for object_index in 0..num_objects {
        let (size, obj_type) = read_size(&mut bufreader, false).context("read object size fail")?;
        eprintln!("object size: {}", size);
        eprintln!("object_type: {:?}", obj_type);
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
                    eprintln!(
                        "Object {} is of type {:?} with hash {:?}",
                        object_index,
                        object.kind,
                        hex::encode(hash)
                    );
                    hashmap.insert(hex::encode(hash), object);
                }
                ObjectType::RefDelta => {
                    // 读取 base object id
                    let mut base_object_id = [0; 20];
                    bufreader
                        .read_exact(&mut base_object_id)
                        .context("read base object id fail")?;
                    eprintln!("base_object_id: {:?}", hex::encode(base_object_id));

                    let hex_base_object_id = hex::encode(base_object_id);
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
                                copy_size |= (b as usize);
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
                    eprintln!(
                        "RefDelta Object {} is of type {:?} with hash {:?}",
                        object_index,
                        object.kind,
                        hex::encode(hash)
                    );
                    hashmap.insert(hex::encode(hash), object);
                }
                ObjectType::OfsDelta => {
                    unimplemented!()
                }
            }
        }

        // println!("object1: {}", String::from_utf8_lossy(&object1));
        // break;
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
) -> Result<Vec<u8>, anyhow::Error> {
    let status = resp.status();
    eprintln!("url: {}, status: {}", url, status);
    match status {
        StatusCode::OK => Ok(resp.bytes().await?.to_vec()),
        StatusCode::NOT_FOUND => {
            eprintln!("repository not found");
            bail!("repository not found");
        }

        _ => {
            eprintln!("clone failed");
            bail!("clone failed");
        }
    }
}

fn validate_response(body: &[u8]) -> Result<(), anyhow::Error> {
    // 确保至少有5个字节
    if body.len() < 5 {
        anyhow::bail!("Response too short");
    }

    // 读取前五个字节并转为字符串
    let first_five = &body[..5];
    let first_five_str = str::from_utf8(first_five)?;

    // 定义正则 ^[0-9a-f]{4}#
    let re = Regex::new(r"^[0-9a-f]{4}#")?;

    // 匹配
    if !re.is_match(first_five_str) {
        anyhow::bail!("Response validation failed");
    }
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
        if body.len() < 4 {
            anyhow::bail!("Incomplete pkt-line length");
        }

        // 读取前 4 个字节作为长度
        let len_str = str::from_utf8(&body[..4])?;
        let mut len = usize::from_str_radix(len_str, 16)?;
        eprintln!("pkt-line length: {}", len);
        // 长度为 0 表示 flush
        if len == 0 {
            body = &body[4..];
            continue;
        }

        if len < 4 || body.len() < len {
            anyhow::bail!("Invalid pkt-line length");
        }

        // 提取 pkt-line 内容（不包含长度字段）
        let content = &body[4..len];
        let content_str = str::from_utf8(content)?.to_string();
        pkt_lines.push(content_str.clone());
        eprint!("pkt-line content: {}", content_str);

        // 移动到下一个 pkt-line
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
        let repo_url = "https://github.com/learn-rust-projects/build-your-own-git";
        let result = invoke(repo_url.to_string()).await;
        if let Err(e) = &result {
            eprintln!("clone error: {:?}", e);
        }
        assert!(result.is_ok());
    }
}
