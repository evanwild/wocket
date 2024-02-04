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
                .write_all(response.as_bytes())
                .await
                .expect("failed to write data to socket");

            if bad_req {
                return;
            }

            done_handshake = true;
            continue;
        }

        // Not a handshake, treat it as a WebSocket frame
        match parse_ws_frame(&buf[0..n]) {
            Err(_) => return,
            Ok(message) => {
                let response = write_ws_frame(&message);

                // Echo back the message
                socket
                    .write_all(&response)
                    .await
                    .expect("failed to write data to socket");
            }
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

fn parse_ws_frame(frame_buf: &[u8]) -> Result<Vec<u8>, &'static str> {
    if frame_buf.len() < 2 {
        return Err("Reached end of frame while parsing");
    }

    let fin_bit = frame_buf[0] >> 7;
    if fin_bit != 1 {
        return Err("FIN bit not 1, payload is fragmented which we don't support");
    }

    let opcode = frame_buf[0] & 0xF;
    if opcode != 0x2 {
        return Err("Opcode field is not 0x2 which means the message is not binary");
    }

    let mask_bit = frame_buf[1] >> 7;
    if mask_bit != 1 {
        return Err("Mask bit not 1, should never happen");
    }

    let mut payload_len = (frame_buf[1] & 0b0111_1111) as u16;
    let mut mask_idx = 2;

    if payload_len == 127 {
        return Err("Message longer than 65535 bytes, not supported");
    }

    if payload_len == 126 {
        // This means the real length does not fit in 7 bits, and it is a 16 bit
        // unsigned integer starting at frame_buf[2]
        if frame_buf.len() < 4 {
            return Err("Reached end of frame while parsing");
        }

        payload_len = ((frame_buf[2] as u16) << 8) | (frame_buf[3] as u16);
        mask_idx = 4;
    }

    let masking_key = &frame_buf[mask_idx..mask_idx + 4];
    let payload = &frame_buf[mask_idx + 4..];

    let mut message = vec![];
    for i in 0..(payload_len as usize) {
        message.push(payload[i] ^ masking_key[i % 4]);
    }

    Ok(message)
}

fn write_ws_frame(message: &[u8]) -> Vec<u8> {
    let mut frame = vec![];

    // FIN bit is 1, opcode field is binary (0x2)
    frame.push(0b1000_0010);

    // Push message length
    if message.len() <= 125 {
        frame.push(message.len() as u8);
    } else {
        frame.push(126);
        frame.push((message.len() >> 8) as u8);
        frame.push((message.len() & 0xFF) as u8);
    }

    frame.extend_from_slice(message);

    frame
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

    #[test]
    fn parse_short() {
        let mut buf = vec![];
        buf.push(0b1000_0010); // FIN bit 1, opcode type binary (0x2)
        buf.push(0b1000_0101); // Mask bit 1, length 5

        // 4 byte masking key (random)
        buf.push(0x12);
        buf.push(0x34);
        buf.push(0xab);
        buf.push(0xcd);

        buf.push(b'H' ^ 0x12);
        buf.push(b'e' ^ 0x34);
        buf.push(b'l' ^ 0xab);
        buf.push(b'l' ^ 0xcd);
        buf.push(b'o' ^ 0x12);

        let expected = b"Hello".to_vec();

        assert_eq!(parse_ws_frame(&buf), Ok(expected));
    }
}
