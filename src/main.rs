use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use zero2prod::email_client::EmailClient;

use tracing::level_filters::LevelFilter;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use zero2prod::{configuration::get_configuration, startup::run};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), LevelFilter::INFO, std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration");
    let connection_pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.database.with_db());
    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );

    let email_config = configuration.email_client;
    let sender = email_config
        .sender()
        .expect("The provided sender email is not valid.");
    let email_client = EmailClient::new(email_config.base_url, email_config.secret, sender);

    tracing::info!("Available on address {}", &address);
    let tcp_listener = TcpListener::bind(address)?;

    run(tcp_listener, connection_pool, email_client)?.await
}
