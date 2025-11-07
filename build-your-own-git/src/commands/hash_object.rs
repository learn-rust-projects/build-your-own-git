use std::{
    env,
    ffi::CStr,
    future,
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{ArgGroup, Parser, Subcommand};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use futures::future::{join_all, ok};
use sha1::{Digest, Sha1};
use tempfile::NamedTempFile;
use tokio::fs;

pub(crate) async fn hash_and_compress_file(
    path: &PathBuf,
    write: bool,
) -> Result<String, anyhow::Error> {
    let mut object = crate::objects::file_to_object(path)?;
    let hash = match write {
        true => object
            .write_object()
            .await
            .context("stream file into blob object failed")?,
        false => object
            .compute_hash(std::io::sink(), false)
            .await
            .context("stream file into blob object failed")?,
    };
    Ok(hex::encode(hash))
}

/// 计算多个文件的哈希并返回空格分隔的结果字符串
pub(crate) async fn hash_multiple_files(
    files: &[PathBuf],
    write: bool,
) -> Result<String, anyhow::Error> {
    // 并行处理所有文件的哈希计算
    let results = join_all(files.iter().map(|f| hash_and_compress_file(f, write))).await;

    // 处理结果，收集所有哈希值
    let hashes = results.into_iter().collect::<Result<Vec<_>, _>>()?;

    // 将哈希值用空格连接
    Ok(hashes.as_slice().join(" "))
}
