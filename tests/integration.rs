use futures::{stream::FuturesUnordered, StreamExt};
use httptest::{matchers::request::method_path, responders::status_code, Expectation, ServerBuilder};
use hyper::StatusCode;
use log::info;
use pretty_assertions::assert_eq;
use reqwest::ClientBuilder;
use serde_json::{json, Value};
use std::{net::SocketAddr, thread::sleep, time::Duration};

mod common;

#[tokio::test]
async fn socks5_proxy_single_client() {
    common::init_logging();

    let lurk_server_addr = "127.0.0.1:32001".parse::<SocketAddr>().unwrap();
    let http_server_addr = "127.0.0.1:32002".parse::<SocketAddr>().unwrap();

    // Run proxy
    let lurk_handle = common::spawn_lurk_server(lurk_server_addr).await;

    // Run HTTP server in the background
    let http_server = ServerBuilder::new()
        .bind_addr(http_server_addr)
        .run()
        .expect("Unable to bind HTTP server");

    http_server.expect(Expectation::matching(method_path("GET", "/hello_world")).respond_with(status_code(200)));

    // Run HTTP client through Lurk proxy
    let http_client = ClientBuilder::new()
        .proxy(common::socks5_proxy(lurk_server_addr))
        .build()
        .expect("Unable to build HTTP client through supplied proxy");

    // Send GET request
    let response = http_client
        .get(http_server.url_str("/hello_world").to_string())
        .send()
        .await
        .expect("Unable to send GET request to HTTP server through proxy");

    assert_eq!(200, response.status());

    lurk_handle.abort();
    drop(http_server);
    sleep(Duration::from_millis(1000));
}

#[tokio::test]
async fn socks5_proxy_multiple_clients() {
    common::init_logging();

    let num_clients = 100;
    let generated_data_len = 1024;
    let lurk_server_addr = "127.0.0.1:32003".parse::<SocketAddr>().unwrap();
    let echo_server_addr = "127.0.0.1:32004".parse::<SocketAddr>().unwrap();

    // Run Lurk proxy.
    let lurk_handle = common::spawn_lurk_server(lurk_server_addr).await;

    // Run echo server. Data sent to this server will be proxied through Lurk
    // instance spawned above.
    let echo_handle = common::spawn_echo_server(echo_server_addr).await;

    // Spawn clients and "ping-pong" data through lurk proxy.
    let client_tasks: FuturesUnordered<_> = (0..num_clients)
        .map(|i| async move {
            info!("Started client #{i:}");
            common::ping_pong_data_through_socks5(echo_server_addr, lurk_server_addr, generated_data_len).await;
            info!("Finished client #{i:}");
        })
        .collect();

    // Await all clients to complete.
    client_tasks.collect::<()>().await;

    // Shutdown listeners.
    echo_handle.abort();
    lurk_handle.abort();

    sleep(Duration::from_millis(1000));
}

#[tokio::test]
async fn http_endpoint_healthcheck() {
    common::init_logging();

    let http_endpoint_addr = "127.0.0.1:32005".parse::<SocketAddr>().unwrap();
    let http_endpoint = common::spawn_http_api_endpoint(http_endpoint_addr).await;

    let http_client = ClientBuilder::new().build().expect("Unable to build HTTP client");

    let response = http_client
        .get(format!("http://{}/healthcheck", http_endpoint_addr))
        .send()
        .await
        .expect("Unable to send healthcheck GET request");

    assert_eq!(StatusCode::OK, response.status());

    let body_bytes = response.bytes().await.unwrap();
    let body_value: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(*body_value.get("uptime_secs").unwrap(), json!(null));
    assert_eq!(*body_value.get("started_utc_ts").unwrap(), json!(null));

    http_endpoint.abort();
    sleep(Duration::from_millis(1000));
}
