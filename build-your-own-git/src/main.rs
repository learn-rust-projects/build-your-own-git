#[allow(unused_imports)]
pub(crate) mod commands;
pub(crate) mod objects;
use std::{env, path::PathBuf};

use clap::{ArgGroup, Parser, Subcommand};
use tokio::fs;
#[derive(Parser)]
#[command(
    //name ="myapp", --version will show name
    version ,
    // about = "Git in Rust",
    // long_about = "Git in Rust"
     arg_required_else_help = true   // 没提供参数时显示帮助
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand, Debug)]
enum Commands {
    /// does testing things
    Init,
    /// hash
    #[command(group(
        ArgGroup::new("input")
            .required(false)              // 必须提供一个输入方式
            .args(&["files", "stdin"])  // 文件模式或 stdin 模式
    )  // 没提供参数时显示帮助
    )]
    HashObject {
        #[arg(short = 'w', long = "write")]
        write: bool,
        // /// 输入类型: blob, tree, commit, tag
        // #[arg(short = 't', long = "type", default_value = "blob")]
        // // Clap 默认必填：用户必须提供，否则命令行解析会报错。
        // path: Option<String>,
        /// 文件列表（文件模式）
        #[arg(num_args = 1..)]
        // 不是 Option，但因为是 Vec，可以为空
        // Clap 默认允许为空 Vec，除非明确设置 num_args = 1.. 或者加 required = true
        files: Vec<PathBuf>,

        /// 从标准输入读取内容（stdin 模式）
        #[arg(long = "stdin")]
        stdin: bool,
    },
    #[command(group(
            ArgGroup::new("mode")
                .required(true)              // 必须提供一个操作模式
                .args(&["pretty", "type", "size", "exists"])  // 这四个参数互斥
        ))]
    CatFile {
        /// 操作类型: pretty-print
        #[arg(short = 'p', long = "pretty")]
        pretty: bool,

        /// 操作类型: type
        #[arg(short = 't', long = "type")]
        _object_type: bool,

        /// 操作类型: size
        #[arg(short = 's', long = "size")]
        _size: bool,

        /// 操作类型: exists
        #[arg(short = 'e', long = "exists")]
        _exists: bool,

        /// 对象 SHA-1 或引用
        object: String,
    },
    LsTree {
        #[arg(long = "name-only")]
        name_only: bool,

        /// tree 对象 SHA-1 或引用
        tree_sha: String,
    },
    WriteTree,
    CommitTree {
        /// tree 对象 SHA-1 或引用
        tree_sha: String,

        #[arg(short = 'm', long = "message")]
        message: String,

        #[arg(short = 'p', long = "parent")]
        parent: Option<String>,
    },
    Commit {
        #[clap(short = 'm')]
        message: String,
    },
    Clonne {
        /// 仓库 URL
        repo_url: String,
    },
}
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // You can use print statements as follows for debugging, they'll be visible
    // when running tests.
    // eprintln!("Logs from your program will appear here!");
    let mut args = env::args();
    let _program = args.next(); // 跳过程序名
    let _first_arg = args.next(); // 获取用户输入的第一个参数（可能是子命令）

    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Init) => {
            fs::create_dir(".git").await?;
            fs::create_dir(".git/objects").await?;
            fs::create_dir(".git/refs").await?;
            fs::write(".git/HEAD", "ref: refs/heads/main\n").await?;
            println!("Initialized git directory");
        }
        Some(Commands::HashObject {
            write,
            // path,
            files,
            stdin,
        }) => {
            if !stdin {
                let output = commands::hash_object::hash_multiple_files(&files, write).await?;
                println!("{}", output);
            } else {
                // todo 没有实现
                println!("stdin:{}", stdin);
            }
        }
        Some(Commands::CatFile {
            pretty,
            _object_type,
            _size,
            _exists,
            object,
        }) => commands::cat_file::invoke(&object, pretty).await?,

        Some(Commands::LsTree {
            name_only,
            tree_sha,
        }) => commands::ls_tree::invoke(&tree_sha, name_only).await?,
        Some(Commands::WriteTree) => {
            let hash = commands::write_tree::invoke(PathBuf::from(".")).await?;
            println!("{}", hex::encode(hash));
        }
        Some(Commands::CommitTree {
            tree_sha,
            message,
            parent,
        }) => {
            let hash = commands::commit::invoke_commit_tree(tree_sha, message, parent).await?;
            println!("{}", hex::encode(hash));
        }
        Some(Commands::Commit { message }) => {
            commands::commit::invoke_commit(message).await?;
        }
        Some(Commands::Clonne { repo_url }) => {
            commands::clone::invoke(repo_url).await?;
        }
        // 这行不会执行，因为默认子命令是必须的，除非使用Some(包装)
        _ => println!("No subcommand provided"),
    };
    Ok(())
}
