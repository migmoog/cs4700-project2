use std::{
    path::PathBuf,
    str::FromStr,
};

use crate::ftp::{ControlStream, FtpResponse, setup_control};
use anyhow::{Result, anyhow};
use clap::Subcommand;

use tokio::{fs::File, io::AsyncWriteExt};
use url::Url;

// need this bc Urls can't recognize relative filepaths
#[derive(Debug, Clone)]
pub enum Location {
    Local(PathBuf),
    Remote(Url),
}

impl Location {
    /// Copies a file from a local filesystem to a remote one or vice versa.
    /// Prints responses from the server to stdout. If it returns a reponse
    /// that means a file was copied __from__ the __local filesystem__, **to** the **remote filesystem**
    async fn copy_to(&self, destination: &Location) -> Result<(ControlStream, Option<FtpResponse>)> {
        match (self, destination) {
            (Self::Local(from), Self::Remote(to)) => {
                let mut control = setup_control(&to).await?;
                let file_data = tokio::fs::read(from.to_str().unwrap()).await?;
                let response = control
                    .data_write_command("STOR", to.path(), &file_data)
                    .await?;

                Ok((control, Some(response)))
            }
            // copy from remote to local
            (Self::Remote(from), Self::Local(to)) => {
                let mut control = setup_control(&from).await?;
                let mut file = File::create(to.to_str().unwrap()).await?;
                let data = control.data_read_command("RETR", from.path()).await?;
                file.write_all(&data).await?;
                Ok((control, None))
            }

            (from, to) => return Err(anyhow!("Paths can't be the same: {:?} == {:?}", from, to)),
        }
    }

    fn path(&self) -> &str {
        match self {
            Location::Local(path_buf) => path_buf.to_str().unwrap(),
            Location::Remote(url) => url.path(),
        }
    }
}

impl FromStr for Location {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.starts_with("ftp://") {
            match Url::parse(s) {
                Ok(u) => Ok(Self::Remote(u)),
                Err(e) => Err(anyhow!("Couldn't parse into url: {e}")),
            }
        } else {
            match PathBuf::from_str(s) {
                Ok(p) => Ok(Self::Local(p)),
                Err(e) => Err(anyhow!("Couldn't read pathbuf: {e}")),
            }
        }
    }
}

/// Enum that holds the urls for each file.
/// Note: Urls can be local to the file system or to ftp
#[derive(Subcommand)]
pub enum Operation {
    Ls { url: Url },
    Mkdir { url: Url },
    Rm { url: Url },
    Rmdir { url: Url },
    Cp { from: Location, to: Location },
    Mv { from: Location, to: Location },
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
        }
        Operation::Rm { url } => {
            let mut control = setup_control(&url).await?;
            let (rest, code) = control.command("DELE", url.path()).await?;
            println!("rm: {} {}", code, rest);
            control
        }
        Operation::Rmdir { url } => {
            let mut control = setup_control(&url).await?;
            let (rest, code) = control.command("RMD", url.path()).await?;
            println!("rmdir: {} {}", code, rest);
            control
        }
        Operation::Cp { from, to } => {
            let (control, response) = from.copy_to(&to).await?;

            if let Some((rest, code)) = response {
                println!(
                    "cp from '{:?}' to '{:?}', Response: {} {}",
                    from, to, code, rest
                );
            } else {
                println!("cp from '{:?}' to '{:?}'", to, from);
            }

            control
        }
        Operation::Mv {
            from,
            to,
        } => {
            let (mut control, response) = from.copy_to(&to).await?;

            if let Some((rest, code)) = response {
                println!(
                    "mv from '{:?}' to '{:?}', Response: {} {}",
                    from, to, code, rest
                );
                
                if (200..300).contains(&code) {
                    tokio::fs::remove_file(from.path()).await?;
                } else {
                    return Err(anyhow!("Upload failed ({code} {rest}); keeping local file"));
                }
            } else {
                println!("mv from '{:?}' to '{:?}'", to, from);
                let (rest, code) = control.command("DELE", to.path()).await?;
                println!("\tDELE Response: {} {}", code, rest);
            }

            control
        }
    };

    // quit connection once command is completed
    let (remaining, code) = control.command("QUIT", "").await?;
    println!("Post quit: {} {}", code, remaining);

    Ok(())
}
