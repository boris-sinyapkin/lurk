use httptest::{matchers, responders, Expectation, ServerBuilder};
use lurk::server::LurkServer;
use pretty_assertions::assert_eq;
use reqwest::{ClientBuilder, Proxy};
use std::{net::SocketAddr, thread::sleep, time::Duration};

mod common;

#[tokio::test]
async fn http_tunnel() {
    common::init_logging();

    let lurk_server_addr = "127.0.0.1:32001".parse::<SocketAddr>().unwrap();
    let http_server_addr = "127.0.0.1:32002".parse::<SocketAddr>().unwrap();

    // Run proxy
    let proxy_future = tokio::spawn(async move {
        LurkServer::new(lurk_server_addr, false)
            .run()
            .await
            .expect("Error during proxy server run")
    });

    // Yeild execution untill server binds
    tokio::task::yield_now().await;

    // Run HTTP server in the background
    let http_server = ServerBuilder::new()
        .bind_addr(http_server_addr)
        .run()
        .expect("Unable to bind HTTP server");

    http_server.expect(
        Expectation::matching(matchers::request::method_path("GET", "/hello_world"))
            .respond_with(responders::status_code(200)),
    );

    // Run HTTP client through Lurk proxy
    let http_proxy =
        Proxy::http(format!("socks5://{}", lurk_server_addr)).expect("Unable to supply proxy to HTTP client");
    let http_client = ClientBuilder::new()
        .proxy(http_proxy)
        .build()
        .expect("Unable to build HTTP client through supplied proxy");

    // Send GET request
    let response = http_client
        .get(http_server.url_str("/hello_world").to_string())
        .send()
        .await
        .expect("Unable to send GET request to HTTP server through proxy");

    assert_eq!(200, response.status());

    proxy_future.abort();
    sleep(Duration::from_millis(1000));
}