use crate::command::Operation;
use anyhow::Result;
use clap::Parser;

mod command;
mod ftp;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    command::run(args.operation).await
}
