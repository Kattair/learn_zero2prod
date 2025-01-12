use zero2prod::startup::Application;

use tracing::level_filters::LevelFilter;
use zero2prod::configuration::get_configuration;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = get_subscriber("zero2prod".into(), LevelFilter::INFO, std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration");

    let application = Application::build(&configuration).await?;
    let _ = application.run_until_stopped().await;

    Ok(())
}
