mod crash_endpoint;
mod crash_overview;
mod utils;
mod webui;

use crate::{crash_endpoint::run_crash_endpoint, webui::run_webui};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    path: PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    tracing_subscriber::fmt::init();
    let path_clone = args.path.clone();
    let crash_endpoint = tokio::spawn(async move { run_crash_endpoint(path_clone).await });
    let webui = tokio::spawn(async move { run_webui(args.path).await });
    let _ = tokio::join!(crash_endpoint, webui);
}
