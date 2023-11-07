use secrecy::ExposeSecret;

use crate::{configuration::RestApiTokenCredentials, domain::SubscriberEmail};

pub struct EmailClient {
    http_client: reqwest::Client,
    base_url: reqwest::Url,
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
            base_url: reqwest::Url::parse(&base_url).unwrap(),
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
        let url = self.base_url.join("send").map_err(|e| e.to_string())?;
        let message = Message {
            from: Recipient { email: self.sender.as_ref().to_owned(), name: None },
            to: Recipient { email: recipient.as_ref().to_owned(), name: None },
            subject: subject.to_owned(),
            html_part: html_body.to_owned(),
            text_part: text_body.to_owned(),
        };
        let builder = self.http_client.post(url)
            .json(&message);
        // if let Some(credentials) = &self.credentials {
        //     builder = builder.basic_auth(
        //         credentials.token.expose_secret(),
        //         Some(credentials.secret.expose_secret()),
        //     );
        // }

        Ok(())
    }
}
#[derive(serde::Serialize)]
struct Message {
    from: Recipient,
    to: Recipient,
    subject: String,
    text_part: String,
    html_part: String,
}

#[derive(serde::Serialize)]
struct Recipient {
    email: String,
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::EmailClient;
    use crate::domain::SubscriberEmail;
    use fake::{
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
        Fake,
    };
    use wiremock::{matchers::any, Mock, ResponseTemplate};

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        let mock_server = wiremock::MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let email_client = EmailClient::new(mock_server.uri(), None, sender);

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let body: String = Paragraph(1..10).fake();

        let _ = email_client
            .send_email(&subscriber_email, &subject, &body, &body)
            .await;
    }
}
