use wiremock::{http::Method, matchers::{any, method, path}, Mock, ResponseTemplate};

use crate::helpers::{spawn_app, TestApp};

#[tokio::test]
pub async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // assert no request is fired against Mailtrap
        .mount(&app.email_server)
        .await;

    let newsletter_json_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "test": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });

    let response = reqwest::Client::new()
        .post(&format!("http://{}/newsletters", &app.app_address))
        .json(&newsletter_json_body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status().as_u16(), 200);
}

async fn create_unconfirmed_subscriber(app: &TestApp) {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    // prevent trying to send a confirmation email to Mailtrap
    let _mock_guard = Mock::given(method(Method::Post))
        .and(path(""))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();
}