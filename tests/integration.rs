mod common;

mod socks5_proxy {

    use crate::common::{
        self,
        listeners::{self, cancel_listener, AsyncListener},
        next_available_address, utils,
    };
    use futures::{stream::FuturesUnordered, StreamExt};
    use httptest::{matchers::request::method_path, responders::status_code, Expectation, ServerBuilder};
    use log::info;

    #[tokio::test]
    async fn single_client() {
        common::init_logging();

        let lurk_server_addr = next_available_address();
        let http_server_addr = next_available_address();

        // Run proxy
        let lurk = listeners::LurkServerListener::new(lurk_server_addr);
        let lurk = lurk.run().await;

        // Run HTTP server in the background
        let http_server = ServerBuilder::new()
            .bind_addr(http_server_addr)
            .run()
            .expect("Unable to bind HTTP server");

        http_server.expect(Expectation::matching(method_path("GET", "/hello_world")).respond_with(status_code(200)));

        // Send GET request
        let response = utils::http::create_http_client_with_proxy(common::socks5_proxy(lurk_server_addr))
            .get(http_server.url_str("/hello_world").to_string())
            .send()
            .await
            .expect("Unable to send GET request to HTTP server through proxy");

        assert_eq!(200, response.status());

        cancel_listener!(lurk);
    }

    #[tokio::test]
    async fn multiple_clients() {
        common::init_logging();

        let num_clients = 100;
        let lurk_server_addr = next_available_address();
        let echo_server_addr = next_available_address();

        // Run Lurk proxy.
        let lurk = listeners::LurkServerListener::new(lurk_server_addr);
        let lurk = lurk.run().await;

        // Run echo server. Data sent to this server will be proxied through Lurk
        // instance spawned above.
        let echo = listeners::tcp_echo_server::TcpEchoServer::bind(echo_server_addr).await;
        let echo = echo.run().await;

        // Spawn clients and "ping-pong" data through lurk proxy.
        let client_tasks: FuturesUnordered<_> = (0..num_clients)
            .map(|i| async move {
                info!("Started client #{i:}");
                common::ping_pong_data_through_socks5(echo_server_addr, lurk_server_addr).await;
                info!("Finished client #{i:}");
            })
            .collect();

        // Await all clients to complete.
        client_tasks.collect::<()>().await;

        cancel_listener!(lurk);
        cancel_listener!(echo);
    }
}

mod http_proxy {

    use crate::common::{self, next_available_address, utils::http::create_http_client};

    #[tokio::test]
    async fn single_client_connect() {
        common::init_logging();

        let echo_server_addr = next_available_address();

        // Spawn HTTP echo server
        let (handle, token) = common::spawn_http_echo_server(echo_server_addr).await;

        // Send GET request
        let response = create_http_client()
            .get(format!("http://{echo_server_addr}/echo"))
            .send()
            .await
            .expect("Unable to send GET request to HTTP server");

        assert_eq!(200, response.status());

        token.cancel();
        handle.await.unwrap();
    }
}

mod api_endpoint {

    use crate::api_endpoint::listeners::cancel_listener;
    use crate::common::{
        self,
        listeners::{self, AsyncListener},
    };
    use crate::common::{next_available_address, utils};
    use hyper::StatusCode;
    use serde_json::{json, Value};

    #[tokio::test]
    async fn healthcheck() {
        common::init_logging();

        let http_endpoint_addr = next_available_address();
        let http_endpoint = listeners::LurkHttpEndpointListener::new(http_endpoint_addr);
        let http_endpoint = http_endpoint.run().await;

        let response = utils::http::create_http_client()
            .get(format!("http://{}/healthcheck", http_endpoint_addr))
            .send()
            .await
            .expect("Unable to send healthcheck GET request");

        assert_eq!(StatusCode::OK, response.status());

        let body_bytes = response.bytes().await.unwrap();
        let body_value: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(*body_value.get("uptime_secs").unwrap(), json!(null));
        assert_eq!(*body_value.get("started_utc_ts").unwrap(), json!(null));

        cancel_listener!(http_endpoint);
    }
}
