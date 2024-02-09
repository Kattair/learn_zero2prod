use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use secrecy::{ExposeSecret, Secret};

use crate::domain::SubscriberEmail;

pub struct EmailClient {
    http_client: reqwest::Client,
    api_url: reqwest::Url,
    sender: SubscriberEmail,
}

impl EmailClient {
    pub fn new(
        api_url: String,
        credentials: Option<Secret<String>>,
        sender: SubscriberEmail,
        timeout: Duration,
    ) -> EmailClient {
        let mut headers = HeaderMap::new();
        if let Some(secret) = credentials {
            let mut auth_header =
                HeaderValue::from_str(&format!("Bearer {}", secret.expose_secret())).unwrap();
            auth_header.set_sensitive(true);
            headers.insert(AUTHORIZATION, auth_header);
        }
        let http_client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .build()
            .unwrap();
        Self {
            http_client,
            api_url: reqwest::Url::parse(api_url.as_ref()).unwrap(),
            sender,
        }
    }

    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html: &str,
        text: &str,
    ) -> Result<(), reqwest::Error> {
        let message = Message {
            from: Recipient {
                email: self.sender.as_ref(),
                name: None,
            },
            to: Recipient {
                email: recipient.as_ref(),
                name: None,
            },
            subject,
            html,
            text,
        };
        self.http_client
            .post(self.api_url.as_ref())
            .json(&message)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
struct Message<'a> {
    from: Recipient<'a>,
    to: Recipient<'a>,
    subject: &'a str,
    text: &'a str,
    html: &'a str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
struct Recipient<'a> {
    email: &'a str,
    name: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::EmailClient;
    use crate::domain::SubscriberEmail;
    use claim::{assert_err, assert_ok};
    use fake::{
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
        Fake, Faker,
    };
    use secrecy::{ExposeSecret, Secret};
    use serde_json::Value;
    use wiremock::{
        http::Method,
        matchers::{any, bearer_token, header, method, path},
        Mock, ResponseTemplate,
    };

    struct SendEmailBodymatcher {}

    impl wiremock::Match for SendEmailBodymatcher {
        fn matches(&self, request: &wiremock::Request) -> bool {
            let result = request.body_json::<Value>();
            if let Ok(body) = result {
                body.get("from").is_some()
                    && body.get("to").is_some()
                    && body.get("subject").is_some()
                    && body.get("text").is_some()
                    && body.get("html").is_some()
            } else {
                false
            }
        }
    }

    fn email_client(api_url: String) -> (EmailClient, Secret<String>) {
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let secret = Secret::new(Faker.fake());

        (
            EmailClient::new(
                api_url,
                Some(secret.to_owned()),
                sender,
                Duration::from_millis(200),
            ),
            secret,
        )
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn body() -> String {
        Paragraph(1..10).fake()
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        let mock_server = wiremock::MockServer::start().await;
        let (email_client, secret) = email_client(mock_server.uri());

        Mock::given(method(Method::Post))
            .and(path("/"))
            .and(bearer_token(secret.expose_secret()))
            .and(header("Content-Type", "application/json"))
            .and(SendEmailBodymatcher {})
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let response = email_client
            .send_email(&email(), &subject(), &body(), &body())
            .await;

        assert_ok!(response);
    }

    #[tokio::test]
    async fn send_email_fails_if_server_returns_500() {
        let mock_server = wiremock::MockServer::start().await;
        let (email_client, _) = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let response = email_client
            .send_email(&email(), &subject(), &body(), &body())
            .await;

        assert_err!(response);
    }

    #[tokio::test]
    async fn send_email_fails_if_request_timeouts() {
        let mock_server = wiremock::MockServer::start().await;
        let (email_client, _) = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500).set_delay(Duration::from_secs(180)))
            .expect(1)
            .mount(&mock_server)
            .await;

        let response = email_client
            .send_email(&email(), &subject(), &body(), &body())
            .await;

        assert_err!(response);
    }
}
