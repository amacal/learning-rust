use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn handle_accepted_connection(mut socket: TcpStream, address: SocketAddr) {
    let mut buffer = [0; 1024];
    let (mut reader, mut writer) = socket.split();

    loop {
        println!("{}: waiting for data", address);
        match reader.read(&mut buffer).await {
            Err(error) => {
                println!("{}: something bad happened - {}", address, error);
            }
            Ok(count) if count == 0 => {
                break println!("{0}: end of stream", address);
            }
            Ok(count) => {
                println!("{}: read {} bytes", address, count);
                match writer.write_all(&buffer[..count]).await {
                    Err(error) => {
                        println!("{}: Something happened: {}", address, error);
                    }
                    Ok(_) => {
                        println!("{}: Wrote {} bytes", address, count);
                    }
                }
            }
        }
    }

    drop(socket);
    println!("{}: remote connection closed", address);
}

#[tokio::main]
async fn main() {
    let address = "127.0.0.1:8080";
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => {
            println!("Listening on {}", address);
            listener
        },
        Err(error) => {
            panic!("{}: local port couldn't be bound - {}", address, error);
        }
    };

    loop {
        match listener.accept().await {
            Err(error) => {
                println!("{}: something bad happened - {}", address, error);
            },
            Ok((socket, address)) => {
                println!("{0}: accepting new connection ...", address);
                tokio::spawn(async move { handle_accepted_connection(socket, address).await });
                println!("{0}: accepting new connection completed", address);
            }
        }
    }
}
