use reqwest::{header, StatusCode};
use sqlx::{PgConnection, Connection};
use zero2prod::configuration::get_configuration;
use std::net::TcpListener;

/// Starts an instance of this app in the background and returns the address it's running at
/// e.g. "127.0.0.1:8000"
fn spawn_app() -> String {
    let tcp_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port.");
    let address = tcp_listener.local_addr().unwrap().to_string();
    let server = zero2prod::startup::run(tcp_listener).expect("Failed to bind address.");

    tokio::spawn(server);

    address
}

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

#[tokio::test]
async fn subscribe_returns_200_when_valid_form_data() {
    let app_address = spawn_app();
    let configuration = get_configuration().expect("Failed to read configuration");
    let mut connection = PgConnection::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres");
    let client = reqwest::Client::new();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(format!("http://{}/subscriptions", &app_address))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(StatusCode::OK, response.status());

    let saved = sqlx::query!("SELECT email, name FROM t_subscriptions")
        .fetch_one(&mut connection)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

#[tokio::test]
async fn subscribe_returns_400_when_data_is_missing() {
    let app_address = spawn_app();
    let client = reqwest::Client::new();
    // TODO: look at 'rstest' crate
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(format!("http://{}/subscriptions", &app_address))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            StatusCode::BAD_REQUEST,
            response.status(),
            "The API did not fail with 400 BAD REQUEST when the payload was {}.",
            error_message
        );
    }
}
