use std::net::TcpListener;

#[tokio::test]
async fn health_check_works() {
    let app_address = spawn_app();
    let client = reqwest::Client::new();

    let response = client
        .get(format!("http://{}/health_check", &app_address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

fn spawn_app() -> String {
    let tcp_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port.");
    let address = tcp_listener.local_addr().unwrap().to_string();
    let server = zero2prod::run(tcp_listener).expect("Failed to bind address.");

    tokio::spawn(server);

    address
}
