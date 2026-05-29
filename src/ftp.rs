pub mod parse;

use anyhow::{Result, anyhow};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

/// Wrapper around a TCP socket to send commands and parse responses
pub struct ControlStream(TcpStream);
impl ControlStream {
    /// Sends an FTP command on the control channel and blocks until a response is received
    pub async fn command(&mut self, name: &str, arg: &str) -> Result<(String, u32)> {
        let command_str = format!("{name} {arg}\r\n");
        self.0.write_all(command_str.as_bytes()).await?;

        let mut response_bytes = Vec::new();
        loop {
            let mut buffer = [0u8; 526];
            let bytes_read = self.0.read(&mut buffer).await?;
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
}

/// Wrapper around a TCP socket to use a data channel
pub struct DataStream(TcpStream);

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

            // Positive completion reply
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
