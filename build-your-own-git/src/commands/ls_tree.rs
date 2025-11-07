use std::{
    any, env,
    ffi::CStr,
    io::{BufRead, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{ArgGroup, Parser, Subcommand};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use tempfile::NamedTempFile;
use tokio::fs;

use crate::objects::{Kind, Mode, hash_to_reader};

pub(crate) async fn invoke(path: &str, name_only: bool) -> Result<(), anyhow::Error> {
    let mut hash_object = hash_to_reader(path).await?;
    // 直接使用std::io::copy将内容输出到终端
    match hash_object.kind {
        Kind::Tree => {
            let mut buf = Vec::new();
            let stdout = std::io::stdout();
            // 自带缓冲
            let mut stdout = stdout.lock();
            let mut hashbuf = [0; 20];
            loop {
                let n = hash_object
                    .reader
                    .read_until(0, &mut buf)
                    .context("read next tree object entry")?;
                if n == 0 {
                    break;
                }
                let mode_and_name = CStr::from_bytes_with_nul(&buf)
                    .context("invalid tree entry")?
                    .to_str()
                    .context("invalid tree entry")?;
                // split_once https://github.com/rust-lang/rust/issues/112811
                let (mode, name) = mode_and_name
                    .split_once(' ')
                    .context("split always yields once")?;

                hash_object.reader.read_exact(&mut hashbuf).context("read entry hash fail")?;
                if name_only {
                    writeln!(&mut stdout, "{name}")?;
                } else {
                    let kind: Kind = Mode::from_str(mode)?.into();
                    writeln!(
                        &mut stdout,
                        "{mode:0>6} {} {}  {name}",
                        kind,
                        hex::encode(hashbuf),
                    )?;
                }
                buf.clear();
            }
        }
        _ => anyhow::bail!("we do not know how to print a '{:?}'", hash_object.kind),
    };
    Ok(())
}
