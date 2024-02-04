use std::error::Error;

use base64::prelude::*;

use sha1::Digest;
use sha1::Sha1;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:8080";

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on: {addr}");

    loop {
        let (socket, _) = listener.accept().await?;

        tokio::spawn(handle_client(socket));
    }
}

async fn handle_client(mut socket: TcpStream) {
    let mut buf = vec![0; 1024];
    let mut done_handshake = false;

    loop {
        let n = socket
            .read(&mut buf)
            .await
            .expect("failed to read data from socket");

        if n == 0 {
            return;
        }

        if !done_handshake {
            let (response, bad_req) = handshake_response(&buf[0..n]);

            socket
                .write_all(&response.into_bytes())
                .await
                .expect("failed to write data to socket");

            if bad_req {
                return;
            }

            println!("Successfully completed handshake");
            done_handshake = true;
        } else {
            // TODO parse ws message here...
        }
    }
}

fn handshake_response(request_buf: &[u8]) -> (String, bool) {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);

    req.parse(request_buf)
        .expect("failed to parse http headers");

    let bad_response = String::from("HTTP/1.1 400 Bad Request\r\n\r\n");

    if req.method != Some("GET") {
        return (bad_response, true);
    }

    let key_header = headers
        .iter()
        .find(|&header| header.name == "Sec-WebSocket-Key");

    let key_value = match key_header {
        Some(header) => header.value,
        _ => return (bad_response, true),
    };

    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
        Upgrade: websocket\r\n\
        Connection: Upgrade\r\n\
        Sec-WebSocket-Accept: {}\r\n\r\n",
        accept_value(key_value),
    );

    (response, false)
}

fn accept_value(key_value: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key_value);
    hasher.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let hashed = hasher.finalize();

    BASE64_STANDARD.encode(hashed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accept_value() {
        assert_eq!(
            accept_value(b"dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }
}
