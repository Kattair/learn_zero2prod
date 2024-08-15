use reqwest::StatusCode;
use wiremock::{http::Method, matchers::{any, method, path}, Mock, ResponseTemplate};

use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};

#[tokio::test]
pub async fn newsletters_returns_400_on_invalid_data() {
    let app = spawn_app().await;
    let test_cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>",
                }
            }),
            "missing title"
        ),
        (
            serde_json::json!({
                "title": "Newsletter title",
            }),
            "missing content"
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(invalid_body).await;

        assert_eq!(
            StatusCode::BAD_REQUEST,
            response.status(),
            "The API did not fail with {} when the payload was {}.",
            StatusCode::BAD_REQUEST,
            error_message
        )
    }    
}

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
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });

    let response = app.post_newsletters(newsletter_json_body).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
pub async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1) // assert a single request is fired against Mailtrap
        .mount(&app.email_server)
        .await;

    let newsletter_json_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });

    let response = app.post_newsletters(newsletter_json_body).await;

    assert_eq!(response.status(), StatusCode::OK);
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_links = create_unconfirmed_subscriber(app).await;

    reqwest::get(confirmation_links.html_link)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
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

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    app.get_confirmation_links(&email_request)
}