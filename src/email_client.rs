use crate::{configuration::RestApiTokenCredentials, domain::SubscriberEmail};

pub struct EmailClient {
    http_client: reqwest::Client,
    base_url: String,
    credentials: Option<RestApiTokenCredentials>,
    sender: SubscriberEmail,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        credentials: Option<RestApiTokenCredentials>,
        sender: SubscriberEmail,
    ) -> EmailClient {
        Self {
            http_client: reqwest::Client::new(),
            base_url,
            credentials,
            sender,
        }
    }

    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_body: &str,
        text_body: &str,
    ) -> Result<(), String> {
        Ok(())
    }
}
