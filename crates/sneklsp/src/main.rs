use anyhow::Result;
use clap::{Parser, Subcommand};

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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Lsp) => {
            tracing::info!("Starting sneklsp server...");
            println!("LSP server not yet implemented");
        }
        Some(Commands::Parse { file }) => {
            tracing::info!(?file, "Parsing file");
            // let source = fs::read_
        }
        Some(Commands::Check { file }) => {
            tracing::info!(?file, "Checking file");
            println!("Checker not yet implemented");
        }
        None => {
            println!("sneklsp v{}", env!("CARGO_PKG_VERSION"));
            println!("Run `sneklsp --help` for usage");
        }
    }

    Ok(())
}
