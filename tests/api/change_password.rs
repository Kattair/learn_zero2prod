use uuid::Uuid;

use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn new_password_fields_must_match() {
    // Arrange
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let another_new_password = Uuid::new_v4().to_string();

    // Act - Part 1 - Login
    app.login_test_user().await;

    // Act - Part 2 - Try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": app.test_user.password,
            "new_password": new_password,
            "new_password_check": another_new_password
        }))
        .await;

    // Assert
    assert_is_redirect_to(&response, "/admin/password");

    // Act - Part 3 - Follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page
        .contains("<p><i>You entered two different password - the field values must match.</i></p"))
}

#[tokio::test]
async fn current_password_must_be_valid() {
    // Arrange
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let wrong_current_password = Uuid::new_v4().to_string();

    // Act - Part 1 - Login
    app.login_test_user().await;

    // Act - Part 2 - Try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": wrong_current_password,
            "new_password": new_password,
            "new_password_check": new_password
        }))
        .await;

    // Assert
    assert_is_redirect_to(&response, "/admin/password");

    // Act - Part 3 - Follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>The current password is incorrect.</i></p"))
}

#[tokio::test]
async fn changing_password_works() {
    // Arrange
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();

    // Act - Part 1 - Log in
    let response = app.login_test_user().await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    // Act - Part 2 - Change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &new_password
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    // Act - Part 3 - Follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>Your password has been changed.</i></p>"));

    // Act - Part 4 - Log out
    let response = app.post_logout().await;
    assert_is_redirect_to(&response, "/login");

    // Act - Part 5 - Log in using the new password
    let response = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &new_password
        }))
        .await;
    assert_is_redirect_to(&response, "/admin/dashboard");
}
