use crate::ftp::setup_control;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::env;
use url::Url;

mod ftp;

#[derive(Subcommand)]
enum Operation {
    Ls { url: Url },
    Mkdir { url: Url },
    Rm { url: Url },
    Rmdir { url: Url },
    Cp { arg1: Url, arg2: Url },
    Mv { arg1: Url, arg2: Url },
}

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    operation: Operation,
}

async fn run_command(op: Operation) -> Result<()> {
    let mut control = match op {
        Operation::Ls { url } => {
            let mut control = setup_control(&url).await?;
            match control.command("LIST", url.path()).await {
                Ok((remaining, code)) => {
                    if code != 200 {
                        eprintln!("Not OK response on LS: {} {}", code, remaining);
                    } else {
                        println!("OK response: {}", remaining);
                    }
                }
                Err(e) => eprintln!("Failed to LS: {}", e),
            }

            control
        }
        Operation::Mkdir { url } => todo!(),
        Operation::Rm { url } => todo!(),
        Operation::Rmdir { url } => todo!(),
        Operation::Cp { arg1, arg2 } => todo!(),
        Operation::Mv { arg1, arg2 } => todo!(),
    };

    let (remaining, code) = control.command("QUIT", "").await?;
    println!("Post quit: {} {}", code, remaining);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    run_command(args.operation).await
}

#[cfg(test)]
mod test {
    use super::*;

    // makes an ftp url for testing on my own machine
    fn setup_ftp_url(path: Option<&str>) -> Url {
        let password =
            std::fs::read_to_string("khourypw").expect("Should have my password on this machine");
        let mut ftp_url = format!("ftp://gordon.jer:{}@ftp.4700.network/", password);
        if let Some(path) = path {
            ftp_url.push_str(path);
        }

        Url::parse(&ftp_url).unwrap()
    }

    #[tokio::test]
    async fn test_ls() -> Result<()> {
        let url = setup_ftp_url(None);
        run_command(Operation::Ls { url }).await
    }
}
