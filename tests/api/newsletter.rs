use std::time::Duration;

use fake::{
    faker::{internet::en::SafeEmail, name::en::Name},
    Fake,
};
use reqwest::StatusCode;
use wiremock::{
    http::Method,
    matchers::{any, method, path},
    Mock, MockBuilder, ResponseTemplate,
};

use crate::helpers::{assert_is_redirect_to, spawn_app, ConfirmationLinks, TestApp};

#[tokio::test]
pub async fn newsletter_creation_is_idempotent() {
    // Arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.login_test_user().await;

    when_sending_an_email()
        .respond_with(ResponseTemplate::new(StatusCode::OK))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act - Part 1 - Submit newsletter form
    let form_data = format!(
        "title=Newsletter%20title\
        &plaintext=Newsletter%20body%20as%20plain%20text\
        &html=<p>Newsletter%20body%20as%20HTML</p>\
        &idempotency_key={}",
        uuid::Uuid::new_v4().to_string()
    );
    let response = app.post_newsletters(form_data.clone()).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 2 - Follow the redirect
    let html = app.get_newsletters_html().await;
    assert!(html.contains("<p><i>The newsletter issues has been published!</i></p>"));

    // Act - Part 3 - Submit newsletter form **again**
    let response = app.post_newsletters(form_data).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 4 - Follow the redirect
    let html = app.get_newsletters_html().await;
    assert!(html.contains("<p><i>The newsletter issues has been published!</i></p>"));

    // Mock verifies on Drop that we have sent the newsletter email **once**
}

#[tokio::test]
pub async fn concurrent_form_submission_is_handled_gracefully() {
    // Arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.login_test_user().await;

    when_sending_an_email()
        // Delay to make it possible for a second request to arrive before the first one completes
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act - Submit two newsletter forms concurrently
    let form_data = format!(
        "title=Newsletter%20title\
        &plaintext=Newsletter%20body%20as%20plain%20text\
        &html=<p>Newsletter%20body%20as%20HTML</p>\
        &idempotency_key={}",
        uuid::Uuid::new_v4().to_string()
    );
    let response1 = app.post_newsletters(form_data.clone());
    let response2 = app.post_newsletters(form_data.clone());
    let (response1, response2) = tokio::join!(response1, response2);

    assert_eq!(response1.status(), response2.status());
    assert_eq!(
        response1.text().await.unwrap(),
        response2.text().await.unwrap()
    );

    // Mock verifies on Drop that we have sent the newsletter email **once**
}

#[tokio::test]
pub async fn transient_errors_do_not_cause_duplicate_deliveries_on_retries() {
    // Arrange
    let app = spawn_app().await;
    let form_data = format!(
        "title=Newsletter%20title\
        &plaintext=Newsletter%20body%20as%20plain%20text\
        &html=<p>Newsletter%20body%20as%20HTML</p>\
        &idempotency_key={}",
        uuid::Uuid::new_v4().to_string()
    );

    create_confirmed_subscriber(&app).await;
    create_confirmed_subscriber(&app).await;
    app.login_test_user().await;

    // Act - Part 1 - Submit newsletter form
    when_sending_an_email()
        .respond_with(ResponseTemplate::new(StatusCode::OK))
        .up_to_n_times(1)
        .expect(1)
        .mount(&app.email_server)
        .await;
    when_sending_an_email()
        .respond_with(ResponseTemplate::new(StatusCode::INTERNAL_SERVER_ERROR))
        .up_to_n_times(1)
        .expect(1)
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletters(form_data.clone()).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // Act - Part 2 - Retry submitting form
    when_sending_an_email()
        .respond_with(ResponseTemplate::new(StatusCode::OK))
        .expect(1)
        .named("Delivery retry")
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletters(form_data.clone()).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Mock verifies on Drop that we did not send duplicates
}

#[tokio::test]
pub async fn requests_missing_authorization_are_rejected() {
    let app = spawn_app().await;

    let newsletter_body = "title=Newsletter%20title&plaintext=Newsletter%20body%20as%20plain%20text&html=<p>Newsletter%20body%20as%20HTML</p>";
    let response = app.post_newsletters(newsletter_body.to_owned()).await;

    assert_is_redirect_to(&response, "/login")
}

#[tokio::test]
pub async fn newsletters_returns_400_on_invalid_data() {
    let app = spawn_app().await;
    app.login_test_user().await;

    let test_cases = vec![
        (
            "plaintext=Newsletter%20body%20as%20plain%20text&html=<p>Newsletter%20body%20as%20HTML</p>",
            "missing title",
        ),
        (
            "title=Newsletter%20title",
            "missing content",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(invalid_body.to_owned()).await;

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
    app.login_test_user().await;

    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(StatusCode::OK))
        .expect(0) // assert no request is fired against Mailtrap
        .mount(&app.email_server)
        .await;

    let newsletter_body = format!(
        "title=Newsletter%20title\
        &plaintext=Newsletter%20body%20as%20plain%20text\
        &html=<p>Newsletter%20body%20as%20HTML</p>\
        &idempotency_key={}",
        uuid::Uuid::new_v4().to_string()
    );
    let response = app.post_newsletters(newsletter_body.to_owned()).await;

    assert_is_redirect_to(&response, "/admin/newsletters");
}

#[tokio::test]
pub async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    app.login_test_user().await;

    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(StatusCode::OK))
        .expect(1) // assert a single request is fired against Mailtrap
        .mount(&app.email_server)
        .await;

    let newsletter_body = format!(
        "title=Newsletter%20title\
        &plaintext=Newsletter%20body%20as%20plain%20text\
        &html=<p>Newsletter%20body%20as%20HTML</p>\
        &idempotency_key={}",
        uuid::Uuid::new_v4().to_string()
    );

    let response = app.post_newsletters(newsletter_body.to_owned()).await;

    assert_is_redirect_to(&response, "/admin/newsletters");
}

fn when_sending_an_email() -> MockBuilder {
    Mock::given(path("")).and(method(Method::Post))
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
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();
    let body = format!("name={name}&email={email}")
        .replace(" ", "%20")
        .replace("@", "%40");

    // prevent trying to send a confirmation email to Mailtrap
    let _mock_guard = Mock::given(method(Method::Post))
        .and(path(""))
        .respond_with(ResponseTemplate::new(StatusCode::OK))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.to_string())
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
