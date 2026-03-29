mod app;
mod cache;
mod git;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "git-explorer", about = "Interactive git history explorer")]
struct Cli {
    /// Path to git repository (defaults to current directory)
    path: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Open in timeline view
    Log {
        #[arg(default_value = ".")]
        path: String,
    },
    /// Open blame view for a file
    Blame { file: String },
    /// Open search view with initial query
    Search { query: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let (repo_path, initial_view, blame_file, search_query) = match cli.command {
        Some(Commands::Log { path }) => (path, app::ViewId::Timeline, None, None),
        Some(Commands::Blame { file }) => (
            ".".to_string(),
            app::ViewId::Blame,
            Some(file),
            None,
        ),
        Some(Commands::Search { query }) => (
            ".".to_string(),
            app::ViewId::Search,
            None,
            Some(query),
        ),
        None => (
            cli.path.unwrap_or_else(|| ".".to_string()),
            app::ViewId::Timeline,
            None,
            None,
        ),
    };

    let mut application = app::App::new(&repo_path, initial_view, blame_file, search_query)?;
    application.run().await
}
