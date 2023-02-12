use tokio::time::{sleep, timeout, Duration};

#[tokio::main]
async fn main() {
    let task = tokio::spawn(async {
        println!("Hello from the executor.");
        sleep(Duration::from_secs(5)).await;
        "success"
    });

    match timeout(Duration::from_secs(10), task).await {
        Ok(result) => println!("completed: {}", result.unwrap()),
        Err(_) => println!("failed: timed out!"),
    };
}
