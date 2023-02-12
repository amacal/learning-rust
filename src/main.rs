use tokio::{net::TcpListener};
use tokio::time::{sleep, Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());

    let (mut socket, address) = listener.accept().await.unwrap();
    println!("Accepted connection on {}", address);

    tokio::spawn(async move {
        let (mut reader, mut writer) = socket.split();

        let mut buffer = [0; 1024];
        let n = reader.read(&mut buffer).await.unwrap();

        println!("Read {} bytes", n);

        writer.write_all(&buffer[..n]).await.unwrap();
        println!("Wrote {} bytes", n);

        drop(socket);
        println!("Closed remote connection {}", address);
    });

    sleep(Duration::from_secs(60)).await;
    drop(listener);
}
