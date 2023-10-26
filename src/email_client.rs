use secrecy::Secret;

use crate::domain::SubscriberEmail;

pub struct EmailClient {
    http_client: reqwest::Client,
    base_url: String,
    api_token: Secret<String>,
    api_secret: Secret<String>,
    sender: SubscriberEmail,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        api_token: Secret<String>,
        api_secret: Secret<String>,
        sender: SubscriberEmail,
    ) -> EmailClient {
        Self {
            http_client: reqwest::Client::new(),
            base_url,
            api_token,
            api_secret,
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
