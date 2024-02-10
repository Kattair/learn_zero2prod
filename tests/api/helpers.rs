use once_cell::sync::Lazy;
use std::net::TcpListener;
use zero2prod::email_client::EmailClient;

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
pub async fn spawn_app() -> TestApp {
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