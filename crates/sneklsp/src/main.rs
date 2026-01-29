use std::fs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use sneklsp_ast::AstArena;

#[derive(Parser)]
#[command(name = "sneklsp")]
#[command(version, about = "Python (sneklang) language server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Lsp,
    Parse { file: std::path::PathBuf },
    Tokenize { file: std::path::PathBuf },
    Check { file: std::path::PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Lsp) => {
            tracing::info!("starting sneklsp server...");
            // log to stderr so it doesn't interefere with protocol
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(tracing::Level::DEBUG.into()),
                )
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .init();

            sneklsp_server::run_server()?;
        }

        Some(Commands::Parse { file }) => {
            tracing::info!(?file, "Parsing file");
            let source = fs::read_to_string(&file)?;
            let arena = AstArena::new();

            match sneklsp_parser::parse(&source, &arena) {
                Ok(module) => {
                    println!("Parsed module '{module:#?}'");
                }
                Err(e) => {
                    eprintln!("Parse error: {e}");
                    std::process::exit(1);
                }
            }
        }

        Some(Commands::Tokenize { file }) => {
            tracing::info!(?file, "Tokenizing file");
            let source = fs::read_to_string(&file)?;
            let tokens = sneklsp_lexer::tokenize(&source);

            for token in tokens {
                let start = token.range.start();
                let end = token.range.end();
                let text = &source[start.to_usize()..end.to_usize()];
                println!(
                    "{:?} {:?} @ {}..{}",
                    token.kind,
                    text,
                    start.to_u32(),
                    end.to_u32(),
                );
            }
        }

        Some(Commands::Check { file }) => {
            tracing::info!(?file, "Checking file");
            let source = fs::read_to_string(&file)?;
            let arena = AstArena::new();

            match sneklsp_parser::parse(&source, &arena) {
                Ok(module) => {
                    println!("[+] {} ({} statements)", file.display(), module.body.len());
                }
                Err(e) => {
                    eprintln!("[-] {}: {e}", file.display());
                    std::process::exit(1);
                }
            }
        }

        None => {
            println!("sneklsp v{}", env!("CARGO_PKG_VERSION"));
            println!("Run `sneklsp --help` for usage");
        }
    }

    Ok(())
}
