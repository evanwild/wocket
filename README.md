# <img align="right" src="wocket.png" title="Wocket in My Pocket"> Wocket

A lightweight WebSocket server implementation in Rust, designed for simplicity.

It is built on top of the Tokio runtime, using its asynchronous capabilities to handle multiple WebSocket clients concurrently. Each client is associated with its own task (green thread), allowing for efficient and parallel processing.

## Getting Started

Clone the repo, then:

```sh
cd wocket
cargo run --release
```

This will start a WebSocket server on port 8080.

You can connect to it and send/receive binary message (e.g. in TypeScript):

```ts
const socket = new WebSocket('ws://127.0.0.1:8080');

socket.binaryType = 'arraybuffer';

// The server will echo back whatever message you send it
const message = new Uint8Array([0xC0, 0xFF, 0xEE]);
socket.send(message);
```
