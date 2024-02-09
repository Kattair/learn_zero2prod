use std::net::Ipv4Addr;

use secrecy::{ExposeSecret, Secret};
use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    ConnectOptions,
};

use crate::domain::SubscriberEmail;

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub email_client: EmailClientSettings,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    pub host: Ipv4Addr,
    pub port: u16,
}

#[derive(serde::Deserialize)]
pub struct EmailClientSettings {
    pub api_url: String,
    pub sender_email: String,
    pub secret: Option<Secret<String>>,
}

impl DatabaseSettings {
    pub fn with_db(&self) -> PgConnectOptions {
        self.without_db()
            .database(&self.database_name)
            .log_statements(tracing::log::LevelFilter::Trace)
    }

    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.username)
            .password(self.password.expose_secret())
            .ssl_mode(ssl_mode)
    }
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.to_owned())
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine te current directory");
    let config_dir = base_path.join("configuration");
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| Environment::Local.as_str().to_owned())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");
    let environment_filename = format!("{}.yaml", environment.as_str());
    let settings = config::Config::builder()
        .add_source(config::File::from(config_dir.join("base.yaml")))
        .add_source(config::File::from(config_dir.join(environment_filename)))
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("__")
                .separator("_"),
        )
        .build()?;

    settings.try_deserialize()
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" => Ok(Environment::Local),
            "production" => Ok(Environment::Production),
            other => Err(format!(
                "{} is not a supported environment. \
                Use either 'local' or 'production'.",
                other
            )),
        }
    }
}
