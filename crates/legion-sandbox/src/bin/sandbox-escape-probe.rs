//! Escape probe — attempts writes and network connections for sandbox testing.
//!
//! Usage:
//!   sandbox-escape-probe write <path>     — try to create a file at <path>
//!   sandbox-escape-probe connect <addr>   — try to TCP-connect to <addr>

use std::io::Write;
use std::net::TcpStream;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: sandbox-escape-probe <write|connect> <target>");
        std::process::exit(2);
    }

    match args[1].as_str() {
        "write" => {
            let path = &args[2];
            match std::fs::File::create(path) {
                Ok(mut f) => {
                    let _ = f.write_all(b"escaped");
                    println!("WRITE_OK");
                    std::process::exit(0);
                }
                Err(e) => {
                    println!("WRITE_DENIED: {e}");
                    std::process::exit(1);
                }
            }
        }
        "connect" => {
            let addr = &args[2];
            match TcpStream::connect(addr) {
                Ok(_) => {
                    println!("CONNECT_OK");
                    std::process::exit(0);
                }
                Err(e) => {
                    println!("CONNECT_DENIED: {e}");
                    std::process::exit(1);
                }
            }
        }
        other => {
            eprintln!("unknown command: {other}");
            std::process::exit(2);
        }
    }
}
