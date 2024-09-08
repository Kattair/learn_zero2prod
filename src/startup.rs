use std::net::TcpListener;

use actix_web::{dev::Server, web, App, HttpServer};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing_actix_web::TracingLogger;

use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{confirm_subscription, health_check, home, login, login_form, publish_newsletter, subscribe},
};

pub struct Application {
    port: u16,
    server: Server,
}

pub struct ApplicationBaseUrl(pub String);

impl Application {
    pub fn build(configuration: &Settings) -> Result<Application, std::io::Error> {
        let connection_pool = get_connection_pool(&configuration.database);

        let email_config = &configuration.email_client;
        let sender = email_config
            .sender()
            .expect("The provided sender email is not valid.");
        let timeout = email_config.timeout();
        let email_client = EmailClient::new(
            email_config.api_url.clone(),
            email_config.secret.clone(),
            sender,
            timeout,
        );

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let tcp_listener = TcpListener::bind(address)?;
        let port = tcp_listener.local_addr().unwrap().port();
        let app_base_url = configuration.application.base_url.as_ref();
        let server = run(tcp_listener, connection_pool, email_client, app_base_url)?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.with_db())
}

fn run(
    tcp_listener: TcpListener,
    connection_pool: PgPool,
    email_client: EmailClient,
    app_base_url: &str,
) -> Result<Server, std::io::Error> {
    let connection_pool = web::Data::new(connection_pool);
    let email_client = web::Data::new(email_client);
    let app_base_url = web::Data::new(ApplicationBaseUrl(app_base_url.to_owned()));

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(connection_pool.clone())
            .app_data(email_client.clone())
            .app_data(app_base_url.clone())
            .route("/", web::get().to(home))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route(
                "/subscriptions/confirm",
                web::get().to(confirm_subscription),
            )
            .route("/newsletters", web::post().to(publish_newsletter))
    })
    .listen(tcp_listener)?
    .run();

    Ok(server)
}
