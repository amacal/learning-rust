use std::error::Error;

use hyper::client::HttpConnector;
use hyper::{Body, Client, Request};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let request = Request::builder()
        .method(hyper::http::Method::GET)
        .uri("http://mirror.accum.se/mirror/wikimedia.org/dumps/enwiki/20230401/enwiki-20230401-abstract.xml.gz")
        .body(Body::empty())?;

    let http = HttpConnector::new();
    let client = Client::builder().build::<_, Body>(http);

    let mut response = client.request(request).await?;
    println!("Status: {}", response.status());

    while response.status().is_redirection() {
        let location = response.headers().get(hyper::header::LOCATION).unwrap().to_str().unwrap();
        println!("Following redirect to: {}", location);

        let request = Request::builder()
            .method(hyper::http::Method::GET)
            .uri(location)
            .body(Body::empty())?;

        response = client.request(request).await?;
        println!("Status: {}", response.status());
    }

    let body = response.into_body();
    println!("Body: {:?}", body);

    let bytes = hyper::body::to_bytes(body).await?;
    println!("Bytes: {:?}", bytes);

    Ok(())
}
