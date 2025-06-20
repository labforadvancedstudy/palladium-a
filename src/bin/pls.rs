// Palladium Language Server (pls)
// "Legendary IDE support for Palladium"

use anyhow::Result;
use clap::Parser;
use palladium::lsp::server::LspServer;

/// Palladium Language Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Log to file instead of stderr
    #[arg(short, long)]
    log_file: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    if args.debug {
        if let Some(log_file) = args.log_file {
            // Log to file
            tracing_subscriber::fmt()
                .with_writer(std::fs::File::create(log_file)?)
                .with_target(false)
                .with_level(true)
                .init();
        } else {
            // Log to stderr
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_target(false)
                .with_level(true)
                .init();
        }
    } else {
        // No logging in production mode
        tracing_subscriber::fmt().with_writer(std::io::sink).init();
    }

    tracing::info!("Starting Palladium Language Server");

    // Create and run LSP server
    let mut server = LspServer::new();
    server.run()?;

    tracing::info!("Palladium Language Server stopped");

    Ok(())
}
