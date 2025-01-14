use argon2::password_hash::SaltString;
use argon2::Algorithm::Argon2id;
use argon2::Version::V0x13;
use argon2::{Argon2, Params, PasswordHasher};
use once_cell::sync::Lazy;
use reqwest::redirect::Policy;
use reqwest::{header, Response, StatusCode};
use serde_json::Value;
use wiremock::MockServer;
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
    pub app_port: u16,
    pub connection_pool: PgPool,
    pub email_server: MockServer,
    pub test_user: TestUser,
    pub api_client: reqwest::Client,
}

pub struct TestUser {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: uuid::Uuid::new_v4(),
            username: uuid::Uuid::new_v4().to_string(),
            password: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::new(Argon2id, V0x13, Params::new(15000, 2, 1, None).unwrap())
            .hash_password(self.password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        sqlx::query!(
            r#"
                INSERT INTO t_users (user_id, username, password_hash)
                VALUES ($1, $2, $3)
            "#,
            self.user_id,
            self.username,
            password_hash,
        )
        .execute(pool)
        .await
        .expect("Failed to create test user.");
    }
}

pub struct ConfirmationLinks {
    pub html_link: reqwest::Url,
    pub text_link: reqwest::Url,
}

impl TestApp {
    pub async fn post_login<Body>(&self, body: &Body) -> Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("http://{}/login", &self.app_address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(format!("http://{}/login", &self.app_address))
            .send()
            .await
            .expect("Failed to execute request.")
            .text()
            .await
            .unwrap()
    }

    pub async fn get_admin_dashboard(&self) -> Response {
        self.api_client
            .get(format!("http://{}/admin/dashboard", &self.app_address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_change_password(&self) -> Response {
        self.api_client
            .get(format!("http://{}/admin/password", &self.app_address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_change_password_html(&self) -> String {
        self.get_change_password().await.text().await.unwrap()
    }

    pub async fn post_change_password<Body>(&self, body: &Body) -> Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(format!("Http://{}/admin/password", &self.app_address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn post_subscriptions(&self, body: String) -> Response {
        self.api_client
            .post(format!("http://{}/subscriptions", &self.app_address))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_newsletters(&self, body: serde_json::Value) -> Response {
        self.api_client
            .post(&format!("http://{}/newsletters", &self.app_address))
            .basic_auth(&self.test_user.username, Some(&self.test_user.password))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: Value = serde_json::from_slice(&email_request.body).unwrap();
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .kinds(&[linkify::LinkKind::Url])
                .links(s)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.app_port)).unwrap();
            confirmation_link
        };

        let html_link = get_link(&body["html"].as_str().unwrap());
        let text_link = get_link(&body["text"].as_str().unwrap());

        ConfirmationLinks {
            html_link,
            text_link,
        }
    }
}

/// Starts an instance of this app in the background and returns the address it's running at
/// e.g. "127.0.0.1:8000"
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.email_client.api_url = email_server.uri();
        c
    };

    configure_db(&configuration.database).await;

    let app = Application::build(&configuration)
        .await
        .expect("Failed to build application.");
    let port = app.port();
    let _ = tokio::spawn(app.run_until_stopped());

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let test_app = TestApp {
        app_address: format!("{}:{}", configuration.application.host, port),
        app_port: port,
        connection_pool: get_connection_pool(&configuration.database),
        email_server,
        test_user: TestUser::generate(),
        api_client: client,
    };
    test_app.test_user.store(&test_app.connection_pool).await;
    test_app
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

pub fn assert_is_redirect_to(response: &Response, location: &str) {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get(header::LOCATION).unwrap(), location);
}
