use crate::ftp::setup_control;
use anyhow::Result;
use clap::Subcommand;

use url::Url;

#[derive(Subcommand)]
pub enum Operation {
    Ls { url: Url },
    Mkdir { url: Url },
    Rm { url: Url },
    Rmdir { url: Url },
    Cp { arg1: Url, arg2: Url },
    Mv { arg1: Url, arg2: Url },
}

/// Takes a command provided by the user and runs it via the ftp protocol.
/// fails gracefully.
pub async fn run(op: Operation) -> Result<()> {
    let mut control = match op {
        Operation::Ls { url } => {
            let mut control = setup_control(&url).await?;
            let data = control.data_read_command("LIST", url.path()).await?;
            println!("ls: {}", str::from_utf8(&data).unwrap());
            control
        }
        Operation::Mkdir { url } => {
            let mut control = setup_control(&url).await?;
            let (rest, code) = control.command("MKD", url.path()).await?;
            println!("mkdir: {} {}", code, rest);
            control
        },
        Operation::Rm { url } => todo!(),
        Operation::Rmdir { url } => {
            let mut control = setup_control(&url).await?;
            let (rest, code) = control.command("RMD", url.path()).await?;
            println!("rmdir: {} {}", code, rest);
            control
        }
        Operation::Cp { arg1, arg2 } => todo!(),
        Operation::Mv { arg1, arg2 } => todo!(),
    };

    // quit connection once command is completed
    let (remaining, code) = control.command("QUIT", "").await?;
    println!("Post quit: {} {}", code, remaining);

    Ok(())
}
