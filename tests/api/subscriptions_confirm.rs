use reqwest::StatusCode;
use wiremock::{
    http::Method,
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers;

#[tokio::test]
async fn confirmation_without_token_fails_with_400() {
    let app = helpers::spawn_app().await;

    let response = reqwest::get(format!("http://{}/subscriptions/confirm", &app.app_address))
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn link_returned_by_subscribe_returns_200_ok_if_called() {
    let app = helpers::spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path(""))
        .and(method(Method::Post))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let _ = app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    let response = reqwest::get(confirmation_links.html_link).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn link_returned_by_subscribe_confirms_subscription() {
    let app = helpers::spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path(""))
        .and(method(Method::Post))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let _ = app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    let _ = reqwest::get(confirmation_links.html_link)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let subscription = sqlx::query!("SELECT email, name, status FROM t_subscriptions")
        .fetch_one(&app.connection_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(subscription.email, "ursula_le_guin@gmail.com");
    assert_eq!(subscription.name, "le guin");
    assert_eq!(subscription.status, "confirmed");
}
