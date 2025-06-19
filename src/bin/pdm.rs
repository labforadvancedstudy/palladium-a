// Palladium Package Manager (pdm)
// "The legendary package manager"

use palladium::package::cli;
use palladium::errors::Result;

fn main() -> Result<()> {
    // Initialize logger for better error messages
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(false)
        .init();
    
    // Run the CLI
    cli::run_cli()
}