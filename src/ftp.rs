pub mod parse;

use anyhow::{Result, anyhow};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

/// Wrapper around a TCP socket to send commands and parse responses
pub struct ControlStream(TcpStream);

pub type FtpResponse = (String, u32);

impl ControlStream {
    /// Awaits a response from the control channel
    async fn wait_for_response(&mut self) -> Result<(String, u32)> {
        let mut response_bytes = Vec::new();
        loop {
            let mut buffer = [0u8; 526];
            let bytes_read = self.0.read(&mut buffer).await?;
            if bytes_read == 0 {
                return Err(anyhow!("Connection with the server was terminated"));
            }
            response_bytes.extend_from_slice(&buffer[..bytes_read]);

            match parse::response(str::from_utf8(&response_bytes)?) {
                Ok((rest, response_code)) => return Ok((rest.to_string(), response_code)),
                Err(nom::Err::Incomplete(_)) => {
                    // waiting for full response, continue on reading
                }
                Err(e) => return Err(anyhow!("Parsing failed: {:?}", e)),
            }
        }
    }

    /// Sends an FTP command on the control channel and blocks until a response is received.
    /// Does not open a data channel.
    pub async fn command(&mut self, name: &str, arg: &str) -> Result<FtpResponse> {
        let command_str = format!("{name} {arg}\r\n");
        self.0.write_all(command_str.as_bytes()).await?;
        self.wait_for_response().await
    }

    /// Helper that takes care of PASV command boilerplate
    async fn enter_passive_mode(&mut self) -> Result<TcpStream> {
        let (rest, code) = self.command("PASV", "").await?;
        if code != 227 {
            return Err(anyhow!(
                "Failed to enter passive mode. Response: {} {}",
                code,
                rest
            ));
        }

        // important rust tip for future Jeremy here:
        // error variants from nom hold references so you have to take ownership
        let (_, host) = parse::passive_mode_ip_address(&rest)
            .map_err(|e| anyhow!("failed to parse PASV response: {e}"))?;
        let data_channel = TcpStream::connect(&host).await?;
        Ok(data_channel)
    }

    /// Similar to `command`, but enters passive mode and opens a data channel.
    /// Instead of returning a response code and the remaining message,
    /// it will return the bytes collected off of the data channel
    pub async fn data_read_command(&mut self, name: &str, arg: &str) -> Result<Vec<u8>> {
        let mut data_channel = self.enter_passive_mode().await?;

        let (mut rest, mut code) = self.command(name, arg).await?;
        loop {
            println!("Data Read Command Response: {} {}", code, rest);
            match code {
                100..=199 => {
                    (rest, code) = self.wait_for_response().await?;
                }
                200..=299 => {
                    break;
                }
                c => {
                    return Err(anyhow!(
                        "Got code that a data read couldn't handle: {c} {rest}"
                    ));
                }
            }
        }

        let mut out = Vec::new();
        data_channel.read_to_end(&mut out).await?;

        Ok(out)
    }

    /// Uses a command that writes to a data channel. Passes a slice of 
    /// bytes to be written to the socket. 
    pub async fn data_write_command(
        &mut self,
        name: &str,
        arg: &str,
        file_data: &[u8],
    ) -> Result<FtpResponse> {
        let mut data_channel = self.enter_passive_mode().await?;
        let (rest, code) = self.command(name, arg).await?;
        println!("Data Write Command Response: {} {}", code, rest);
        match code {
            100..=299 => {
                data_channel.write_all(file_data).await?;
                data_channel.shutdown().await?;
                self.wait_for_response().await
            }
            // a refusal from the server, meaning the write should be skipped
            500..=599 => {
                if code == 553 {
                    eprintln!("Couldnt create at path: {}", arg);
                }
                return Ok((rest, code));
            }
            c => {
                return Err(anyhow!(
                    "Got code that a data write couldn't handle: {c} {rest}"
                ));
            }
        }
    }
}

/// Takes an FTP url and attempts to connect to it.
/// Returns a tuple of (control_channel, data_channel)
pub async fn setup_control(url: &Url) -> Result<ControlStream> {
    assert_eq!(url.scheme(), "ftp");

    let mut control = TcpStream::connect(format!(
        "{}:{}",
        url.host_str().expect("Should have a host in FTP url"),
        url.port().unwrap_or(21)
    ))
    .await?;

    // read for hello message
    let mut hello_buffer = [0u8; 1024];
    let bytes_read = control.read(&mut hello_buffer).await?;
    if bytes_read != 0 {
        println!("{:?}", str::from_utf8(&hello_buffer[..bytes_read]));
    } else {
        return Err(anyhow!("Socket disconnected"));
    }

    let mut control = ControlStream(control);
    let mut setup_commands = Vec::new();
    let mut username = url.username();
    if username.is_empty() {
        username = "anonymous";
    }
    setup_commands.push(("USER", username));

    if let Some(password) = url.password() {
        setup_commands.push(("PASS", password));
    }
    // set type to 8bit binary data
    setup_commands.push(("TYPE", "I"));
    // set mode to stream
    setup_commands.push(("MODE", "S"));
    // set to file oriented mode
    setup_commands.push(("STRU", "F"));

    for (command, arg) in setup_commands {
        let (remaining, code) = control.command(command, arg).await?;
        let prefix = match code {
            100..=199 => "Positive Prelim",

            200..=299 => "Positive Completion",

            300..=399 => "Positive Intermediate",

            400..=499 => "Negative Transient",

            500..=599 => {
                return Err(anyhow!(
                    "Permanent Negative Completion on setup command '{} {}'. Response: {} {}",
                    command,
                    arg,
                    code,
                    remaining
                ));
            }
            _ => unreachable!(),
        };

        println!("{}: {} {}", prefix, code, remaining);
    }

    Ok(control)
}
