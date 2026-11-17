use arcs::cli::{Cli, run_cli};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("arcs=info".parse().unwrap()))
        .with_target(false)
        .init();

    let cli = Cli::parse();
    if let Err(e) = run_cli(cli).await {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}