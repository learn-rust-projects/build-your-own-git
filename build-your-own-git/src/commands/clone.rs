use std::any;

use regex::Regex;
use reqwest::StatusCode;

pub(crate) fn invoke(repo_url: String) -> Result<(), anyhow::Error> {
    let repo_url = repo_url.trim_end_matches(".git").trim_end_matches('/');
    let git_url = format!("{}/info/refs?service=git-upload-pack", repo_url);
    let client = reqwest::blocking::Client::new();
    let mut resp = client.get(&git_url).send().unwrap();
    let status = resp.status();
    eprintln!("url: {},status: {}", git_url, status.to_string());

    // 1. 客户必须验证状态码是否为 200 OK 或 200 错误。
    match status {
        StatusCode::OK => {}
        StatusCode::NOT_FOUND => {
            eprintln!("repository not found");
            anyhow::bail!("repository not found");
        }
        _ => {
            eprintln!("clone failed");
            anyhow::bail!("clone failed");
        }
    }
    let mut vec = Vec::new();
    resp.copy_to(&mut vec)?;
    // 2. 客户端必须验证响应实体的前五个字节是否与正则表达式 ^ [ 0-9a-f ] {4}#
    // 匹配。如果此测试失败，客户端不得继续。
    validate_response(&vec)?;

    // 3. 客户端必须将整个响应解析为一系列 pkt-line 记录。
    Ok(())
}

fn validate_response(body: &[u8]) -> Result<(), anyhow::Error> {
    // 确保至少有5个字节
    if body.len() < 5 {
        anyhow::bail!("Response too short");
    }

    // 读取前五个字节并转为字符串
    let first_five = &body[..5];
    let first_five_str = std::str::from_utf8(first_five)?;

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
        let len = usize::from_str_radix(len_str, 16)?;

        // 长度为 0 表示 flush
        if len == 0 {
            break;
        }

        if len < 4 || body.len() < len {
            anyhow::bail!("Invalid pkt-line length");
        }

        // 提取 pkt-line 内容（不包含长度字段）
        let content = &body[4..len];
        let content_str = str::from_utf8(content)?.to_string();
        pkt_lines.push(content_str);

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
