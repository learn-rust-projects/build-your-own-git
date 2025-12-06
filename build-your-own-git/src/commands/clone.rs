use std::{any, f32::consts::E, io::Write, os::linux::raw::stat};

use anyhow::{Context, Result, bail};
use regex::Regex;
use reqwest::StatusCode;

pub(crate) fn invoke(repo_url: String) -> Result<(), anyhow::Error> {
    let repo_url = repo_url.trim_end_matches(".git").trim_end_matches('/');
    let client = reqwest::blocking::Client::new();
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
    let hash = "9671f5a72cac8b4c379b1c35a6af6d10611d620f";

    let clone_url = format!("{}/git-upload-pack", repo_url);

    let mut req_body = Vec::new();
    writeln!(req_body, "0032want {}", hash)?;
    writeln!(req_body, "00000009done")?;

    let mut resp = client
        .post(&clone_url)
        .header("Content-Type", "application/x-git-upload-pack-request")
        .body(req_body)
        .send()
        .unwrap();
    let body = validate_status_and_return_body(&mut resp, &clone_url)?;
    // &[u8] 本身实现了 Read：
    // 写入到一个文件里面
    // TODO 之后需要删除 写入文件中
    let mut file = std::fs::File::create("../packfile_code.bin")?;
    std::io::copy(&mut body.as_slice(), &mut file)?;
    Ok(())
}

/// 校验 Git Upload-Pack 的 HTTP 状态码
pub fn validate_status_and_return_body(
    resp: &mut reqwest::blocking::Response,
    url: &str,
) -> Result<Vec<u8>, anyhow::Error> {
    let status = resp.status();
    eprintln!("url: {}, status: {}", url, status);
    match status {
        StatusCode::OK => {
            let mut vec: Vec<u8> = Vec::new();
            resp.copy_to(&mut vec)?;
            Ok(vec)
        }
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

    #[test]
    fn test_clone() {
        let repo_url = "https://github.com/learn-rust-projects/build-your-own-git";
        let result = invoke(repo_url.to_string());
        assert!(result.is_ok());
    }
}
