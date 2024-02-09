use once_cell::sync::Lazy;
use std::net::TcpListener;
use zero2prod::email_client::EmailClient;

use reqwest::{header, StatusCode};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use tracing_subscriber::filter::LevelFilter;
use uuid::Uuid;

use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = LevelFilter::DEBUG;
    let subscriber_name = "zero2prod_it";

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(
            subscriber_name.into(),
            default_filter_level,
            std::io::stdout,
        );
        init_subscriber(subscriber);
    } else {
        let subscriber =
            get_subscriber(subscriber_name.into(), default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub app_address: String,
    pub connection_pool: PgPool,
}

/// Starts an instance of this app in the background and returns the address it's running at
/// e.g. "127.0.0.1:8000"
async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let tcp_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port.");
    let app_address = tcp_listener.local_addr().unwrap().to_string();

    let mut configuration = get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();
    let connection_pool = configure_db(&configuration.database).await;

    let email_config = configuration.email_client;
    let sender = email_config
        .sender()
        .expect("The provided sender email is not valid.");
    let timeout = email_config.timeout();
    let email_client = EmailClient::new(email_config.api_url, email_config.secret, sender, timeout);

    let server = zero2prod::startup::run(tcp_listener, connection_pool.clone(), email_client)
        .expect("Failed to bind address.");

    tokio::spawn(server);

    TestApp {
        app_address,
        connection_pool,
    }
}

async fn configure_db(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres.");
    connection
        .execute(format!(r#"CREATE DATABASE "{}""#, &config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database.");

    connection_pool
}

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("http://{}/health_check", &app.app_address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_returns_200_when_valid_form_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(format!("http://{}/subscriptions", &app.app_address))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(StatusCode::OK, response.status());

    let saved = sqlx::query!("SELECT email, name FROM t_subscriptions")
        .fetch_one(&app.connection_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

#[tokio::test]
async fn subscribe_returns_404_when_fields_are_present_but_empty() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    // TODO: look at 'rstest' crate
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=le%20guin&email=", "empty email"),
        ("name=&email=", "empty name and email"),
    ];

    for (payload, description) in test_cases {
        let response = client
            .post(format!("http://{}/subscriptions", &app.app_address))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(payload)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            StatusCode::BAD_REQUEST,
            response.status(),
            "The API did not return a {} when the payload was {}",
            StatusCode::BAD_REQUEST,
            description
        )
    }
}

#[tokio::test]
async fn subscribe_returns_400_when_data_is_missing() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    // TODO: look at 'rstest' crate
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(format!("http://{}/subscriptions", &app.app_address))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            StatusCode::BAD_REQUEST,
            response.status(),
            "The API did not fail with {} when the payload was {}.",
            StatusCode::BAD_REQUEST,
            error_message
        );
    }
}
