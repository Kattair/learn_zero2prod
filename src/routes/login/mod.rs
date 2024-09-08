use std::fmt;

use actix_web::{http::header::{self, ContentType}, web::{self, Query}, HttpResponse, ResponseError};
use reqwest::StatusCode;
use secrecy::Secret;
use sqlx::PgPool;

use crate::{authentication::{self, AuthError, Credentials}, error::error_chain_fmt};

#[derive(serde::Deserialize, Debug)]
pub struct LoginFormData {
    username: String,
    password: Secret<String>,
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong.")]
    UnexpectedError(#[from] anyhow::Error),
}

impl fmt::Debug for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(&self, f)
    }
}

impl ResponseError for LoginError {
    fn status_code(&self) -> reqwest::StatusCode {
        StatusCode::SEE_OTHER
    }
    
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        let encoded_error = urlencoding::Encoded::new(self.to_string());
        HttpResponse::build(self.status_code())
            .insert_header((header::LOCATION, format!("/login?error={}", encoded_error)))
            .finish()
    }
}

#[derive(serde::Deserialize)]
pub struct LoginQueryParams {
    error: Option<String>,
}

pub async fn login_form(params: Query<LoginQueryParams>) -> HttpResponse {
    let error_html = match params.0.error {
        None => "".into(),
        Some(error_message) => format!("<p><i>{}</i></p>", htmlescape::encode_minimal(&error_message)),
    };
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(include_str!("login.html"), error_html = error_html))
}

#[tracing::instrument(
    skip(form, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(form: web::Form<LoginFormData>, pool: web::Data<PgPool>) -> Result<HttpResponse, LoginError> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password
    };
    tracing::Span::current()
        .record("username", &tracing::field::display(&credentials.username));
    authentication::validate_credentials(credentials, &pool)
        .await
        .map(|user_id| {
            tracing::Span::current().record("user_id", tracing::field::display(&user_id));
            user_id
        })
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
            AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
        })?;
    Ok(
        HttpResponse::SeeOther()
        .insert_header((header::LOCATION, "/"))
        .finish()
    )
}