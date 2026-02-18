mod app;
mod database;
mod tree;

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser)]
#[command(name = "mdd-ui", about = "Browse MDD diagnostic databases")]
struct Cli {
    /// Path to the MDD file to open
    mdd_file: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    eprintln!("Loading {}...", cli.mdd_file);
    let db = database::load_mdd(&cli.mdd_file)
        .with_context(|| format!("Failed to load: {}", cli.mdd_file))?;

    eprintln!("Building tree...");
    let nodes = tree::build_tree(&db);
    eprintln!("Loaded {} nodes. Starting UI...", nodes.len());

    let mut terminal = ratatui::init();
    let result = app::App::new(nodes).run(&mut terminal);
    ratatui::restore();

    result.context("TUI error")
}
