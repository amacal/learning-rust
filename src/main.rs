use tokio;
use tokio_postgres::{self, NoTls};

use testcontainers::clients;
use testcontainers::images::postgres;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let docker = clients::Cli::default();
    let postgresql = docker.run(postgres::Postgres::default());

    let target = format!(
        "host=127.0.0.1 port={0} user=postgres dbname=postgres",
        postgresql.get_host_port_ipv6(5432),
    );

    let (client, connection) = tokio_postgres::connect(&target, NoTls).await?;

    tokio::spawn(async move {
        match connection.await {
            Ok(_) => println!("completed"),
            Err(error) => println!("failed: {}", error),
        }
    });

    for row in client.query("select 13 as cnt", &[]).await? {
        let cnt: i32 = row.get("cnt");
        println!("{}", cnt);
    }

    Ok(())
}
