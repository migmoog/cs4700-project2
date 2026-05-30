I made this using rust and the tokio runtime.

My implementation involved collecting the elements of the ftp urls (host, username, password, etc),
and then connecting to the server with the initial set of commands in the project description.
Each command spawns the control channel only once, and after every executed command it sends a QUIT message.
For commands that use data channels, they send a PASV command and connect to the host from the response.

My main issues in developing it were the response codes. FTP response codes convey very general information that I found
hard to categorize programitcally. My implementation of writing commands to the socket was very general and took string slices.
