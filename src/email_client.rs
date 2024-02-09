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
    ) -> EmailClient {
        let mut headers = HeaderMap::new();
        if let Some(secret) = credentials {
                let mut auth_header = HeaderValue::from_str(&format!("Bearer {}", secret.expose_secret()))
                    .unwrap();
                auth_header.set_sensitive(true);
                headers.insert(AUTHORIZATION, auth_header);
        }
        let http_client = reqwest::Client::builder()
            .default_headers(headers)
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
        html_body: &str,
        text_body: &str,
    ) -> Result<(), reqwest::Error> {
        let message = Message {
            from: Recipient { email: self.sender.as_ref(), name: None },
            to: Recipient { email: recipient.as_ref(), name: None },
            subject: subject,
            html: html_body,
            text: text_body,
        };
        self.http_client.post(self.api_url.as_ref())
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
    use super::EmailClient;
    use crate::domain::SubscriberEmail;
    use claim::{assert_err, assert_ok};
    use fake::{
        faker::{
            internet::en::SafeEmail, lorem::en::{Paragraph, Sentence}
        },
        Fake, Faker,
    };
    use secrecy::{ExposeSecret, Secret};
    use serde_json::Value;
    use wiremock::{http::Method, matchers::{any, bearer_token, header, method, path}, Mock, ResponseTemplate};

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

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        let mock_server = wiremock::MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let secret = Secret::new(Faker.fake());
        let email_client = EmailClient::new(mock_server.uri(), Some(secret.to_owned()), sender);

        Mock::given(method(Method::Post))
            .and(path("/"))
            .and(bearer_token(secret.expose_secret()))
            .and(header("Content-Type", "application/json"))
            .and(SendEmailBodymatcher{})
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let body: String = Paragraph(1..10).fake();

        let response = email_client
            .send_email(&subscriber_email, &subject, &body, &body)
            .await;

        assert_ok!(response);
    }

    #[tokio::test]
    async fn send_email_fails_if_server_returns_500() {
        let mock_server = wiremock::MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let secret = Secret::new(Faker.fake());
        let email_client = EmailClient::new(mock_server.uri(), Some(secret.to_owned()), sender);

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let body: String = Paragraph(1..10).fake();

        let response = email_client
            .send_email(&subscriber_email, &subject, &body, &body)
            .await;

        assert_err!(response);
    }
}
