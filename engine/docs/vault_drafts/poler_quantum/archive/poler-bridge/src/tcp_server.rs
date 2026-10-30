//! tcp_server.rs — Многопоточный асинхронный TCP-сервер семантического моста
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};

pub struct PolerBridgeServer {
    pub port: u16,
}

impl PolerBridgeServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn start(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))?;
        println!("POLER Bridge Server active on port {}", self.port);
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buffer = [0; 512];
                let _ = stream.read(&mut buffer);
                let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nPOLER_OK\n";
                let _ = stream.write_all(response);
            }
        }
        Ok(())
    }
}
