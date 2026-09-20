//! `mdl` 命令行工具（Phase 4）。Task 4.1 build / 4.2 verify / 4.2 sign。

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use metado_cli::{build_plugin, default_key_path, run_mdl, sign_mdl, verify_mdl, KeyStore};

#[derive(Parser)]
#[command(name = "mdl", version, about = "Metado plugin toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 构建插件目录 -> 签名 .mdl
    Build {
        /// 插件源码目录（须含 mdl.toml）
        dir: PathBuf,
        /// 签名密钥 hex 文件（不存在则生成）
        #[arg(long)]
        key: Option<PathBuf>,
        /// 输出文件（默认 <插件名>.mdl）
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// 校验 .mdl 签名与清单
    Verify {
        file: PathBuf,
    },
    /// 在进程内运行插件（invoke boot 入口）
    Run {
        file: PathBuf,
        /// 模拟授权，可重复（如 --grant http.get.api.external）
        #[arg(long)]
        grant: Vec<String>,
    },
    /// 监听插件目录，变更防抖后重建 + 运行（需 --features watch 编译）
    Watch {
        dir: PathBuf,
        /// 签名密钥 hex 文件
        #[arg(long)]
        key: Option<PathBuf>,
        /// 输出 .mdl（默认 <插件名>.mdl）
        #[arg(long)]
        output: Option<PathBuf>,
        /// 防抖窗口毫秒
        #[arg(long, default_value_t = 500)]
        debounce_ms: u64,
    },
    /// 重新签名（更换签名者）
    Sign {
        file: PathBuf,
        #[arg(long)]
        key: Option<PathBuf>,
    },
}

fn resolve_key(path: &Option<PathBuf>, create_if_missing: bool) -> Result<metado_engine::KeyPair, String> {
    let path = path
        .clone()
        .unwrap_or_else(default_key_path);
    match KeyStore::load(&path) {
        Ok(kp) => Ok(kp),
        Err(err) if err == metado_cli::KeyStoreError::Missing && create_if_missing => {
            eprintln!("no key at {}; generating", path.display());
            KeyStore::generate(&path)
        }
        Err(err) => Err(format!("key at {}: {:?}", path.display(), err)),
    }
}

fn write_mdl(bytes: &[u8], out: &Path) -> Result<(), String> {
    std::fs::write(out, bytes).map_err(|e| format!("write {}: {}", out.display(), e))
}

fn main() {
    let cli = Cli::parse();
    if let Err(msg) = run(cli) {
        eprintln!("error: {}", msg);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Build { dir, key, output } => {
            let kp = resolve_key(&key, true)?;
            let bytes = build_plugin(&dir, &kp)?;
            let info = verify_mdl(&bytes)?;
            let out = output.unwrap_or_else(|| PathBuf::from(format!("{}.mdl", info.plugin_name)));
            write_mdl(&bytes, &out)?;
            println!("built {} (plugin {} signer {})", out.display(), info.plugin_name, shorter(&info.signer));
            Ok(())
        }
        Command::Verify { file } => {
            let bytes = std::fs::read(&file).map_err(|e| format!("read {}: {}", file.display(), e))?;
            let info = verify_mdl(&bytes)?;
            println!("OK {}: plugin {} signer {}", file.display(), info.plugin_name, shorter(&info.signer));
            Ok(())
        }
        Command::Sign { file, key } => {
            let kp = resolve_key(&key, false)?;
            let bytes = std::fs::read(&file).map_err(|e| format!("read {}: {}", file.display(), e))?;
            let re = sign_mdl(&bytes, &kp)?;
            write_mdl(&re, &file)?;
            println!("re-signed {} with signer {}", file.display(), shorter(&metado_engine::signer_id(&kp.public_key_bytes())));
            Ok(())
        }
        Command::Watch {
            dir,
            key,
            output,
            debounce_ms,
        } => {
            #[cfg(feature = "watch")]
            {
                let kp_key = key.clone().unwrap_or_else(default_key_path);
                let out = match output {
                    Some(o) => o,
                    None => PathBuf::from(format!(
                        "{}.mdl",
                        dir.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "plugin".into())
                    )),
                };
                let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                metado_cli::run_watch(&dir, &kp_key, &out, debounce_ms, stop)?;
                Ok(())
            }
            #[cfg(not(feature = "watch"))]
            {
                let _ = (&dir, &key, &output, &debounce_ms);
                Err("mdl watch requires building with --features watch".into())
            }
        }

        Command::Run { file, grant } => {
            let bytes = std::fs::read(&file).map_err(|e| format!("read {}: {}", file.display(), e))?;
            let outcome = run_mdl(&bytes, &grant)?;
            if outcome.invoked {
                println!(
                    "ran {} boot (signer {}) -> {}",
                    outcome.plugin_id,
                    shorter(&outcome.signer),
                    outcome.result.to_json_string().unwrap_or_else(|_| "<value>".into())
                );
            } else {
                println!("{} declares no boot entry; loaded OK", outcome.plugin_id);
            }
            Ok(())
        }
    }
}

fn shorter(id: &str) -> String {
    id.chars().take(12).collect()
}