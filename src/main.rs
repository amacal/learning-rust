use tokio::runtime::Runtime;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio::time::Duration;

fn main() {
    let runtime = Runtime::new().unwrap();

    let task = runtime.spawn(async {
        println!("Hello from the executor.");
        sleep(Duration::from_secs(5)).await;
        "success"
    });

    runtime.block_on(async {
        match timeout(Duration::from_secs(10), task).await {
            Ok(result) => println!("completed: {}", result.unwrap()),
            Err(_) => println!("failed: timed out!"),
        };
    });
}
