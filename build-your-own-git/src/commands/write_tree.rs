use std::{
    env,
    ffi::CStr,
    fs::DirEntry,
    future::{self, Future},
    io::{BufRead, Cursor, Read, Write},
    path::{self, Path, PathBuf},
    pin::Pin,
};

use anyhow::{Context, Ok};
use clap::{ArgGroup, Parser, Subcommand};
use flate2::{
    Compression,
    read::ZlibDecoder,
    write::{self, ZlibEncoder},
};
use futures::{
    future::{join_all, ok},
    stream::Any,
};
use sha1::{Digest, Sha1};
use tempfile::NamedTempFile;
use tokio::fs;

use crate::objects::{self, Kind, Mode, Object};
type TreeFuture =
    Pin<Box<dyn Future<Output = Result<Option<Object<Cursor<Vec<u8>>>>, anyhow::Error>> + Send>>;

pub(crate) fn write_tree(path: PathBuf) -> TreeFuture {
    Box::pin(async move {
        let mut dir = fs::read_dir(path).await.context("open directory failed")?;
        let mut vec = Vec::new();

        while let Some(entry) = dir.next_entry().await.context("read directory failed")? {
            let name = entry.file_name();
            let path = entry.path();
            let mode: Mode = Mode::from_path(&path).await?;
            vec.push((name, path, mode));
        }

        vec.sort_by(|a, b| {
            let afn = a.0.as_encoded_bytes();
            let bfn = b.0.as_encoded_bytes();

            let prefix_cmp = afn
                .iter()
                .zip(bfn.iter())
                .find_map(|(x, y)| if x != y { Some(x.cmp(y)) } else { None });

            if let Some(ord) = prefix_cmp {
                return ord;
            }

            let common_len = afn.len().min(bfn.len());
            let next_byte_or_slash = |bytes: &[u8], len: usize, is_dir: bool| {
                bytes
                    .get(len)
                    .copied()
                    .or(if is_dir { Some(b'/') } else { None })
            };
            let a_next = next_byte_or_slash(afn, common_len, a.2.is_dir());
            let b_next = next_byte_or_slash(bfn, common_len, b.2.is_dir());

            a_next.cmp(&b_next)
        });
        // TODO: 使用bytesmut优化
        let mut tree_object = Vec::new();
        for item in vec {
            let hash = if Mode::Directory == item.2 {
                if item.0 == ".git" {
                    continue;
                }
                invoke(item.1).await?
            } else {
                crate::objects::file_to_object(&item.1)?
                    .write_object(PathBuf::from("."))
                    .await
                    .context("write object failed")?
            };
            tree_object.write_all(item.2.to_bytes())?;
            tree_object.write_all(b" ")?;
            tree_object.write_all(item.0.as_encoded_bytes())?;
            tree_object.write_all(b"\0")?;
            tree_object.write_all(&hash)?;
        }

        if tree_object.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Object {
                kind: Kind::Tree,
                expected_size: tree_object.len() as u64,
                reader: Cursor::new(tree_object),
            }))
        }
    })
}

pub(crate) async fn invoke(path: PathBuf) -> Result<[u8; 20], anyhow::Error> {
    Ok({
        write_tree(path)
            .await
            .context("invoke write tree failed")?
            .ok_or_else(|| anyhow::anyhow!("invoke write tree ok_or_else failed"))?
            .write_object(PathBuf::from("."))
            .await
            .context("invoke write tree  write_object failed")?
    })
}
