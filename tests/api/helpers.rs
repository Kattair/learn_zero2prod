use once_cell::sync::Lazy;
use reqwest::{header, Response};
use zero2prod::startup::{get_connection_pool, Application};

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

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> Response {
        reqwest::Client::new()
            .post(format!("http://{}/subscriptions", &self.app_address))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

/// Starts an instance of this app in the background and returns the address it's running at
/// e.g. "127.0.0.1:8000"
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c
    };

    configure_db(&configuration.database).await;

    let app = Application::build(&configuration).expect("Failed to build application.");
    let port = app.port();
    let _ = tokio::spawn(app.run_until_stopped());

    TestApp {
        app_address: format!("{}:{}", configuration.application.host, port),
        connection_pool: get_connection_pool(&configuration.database),
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
