mod client;
mod config;
mod models;

#[cfg(feature = "cli")]
mod cli;

#[cfg(feature = "tui")]
mod tui;

#[cfg(feature = "cli")]
fn main() -> anyhow::Result<()> {
    use clap::Parser;
    let args = cli::Args::parse();
    cli::run(args)
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("sks3200: built without CLI support (enable the 'cli' feature)");
    std::process::exit(1);
}
