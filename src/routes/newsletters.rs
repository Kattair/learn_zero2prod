use std::fmt::{self, Debug};

use actix_web::{http::header::{self, HeaderMap}, web, HttpRequest, HttpResponse, ResponseError};
use anyhow::{anyhow, Context};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use base64::Engine;
use reqwest::{header::HeaderValue, StatusCode};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{domain::SubscriberEmail, email_client::EmailClient, error::error_chain_fmt, telemetry::spawn_blocking_with_tracing};

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl fmt::Debug for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            PublishError::UnexpectedError(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#)
                    .unwrap();

                response.headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);

                response
            }
        }
    }
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(body, connection_pool, email_client, request),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    connection_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest)
-> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers())
        .map_err(PublishError::AuthError)?;
    tracing::Span::current().record("username", tracing::field::display(&credentials.username));
    let user_id = validate_credentials(credentials, &connection_pool)
        .await?;
    tracing::Span::current().record("user_id", tracing::field::display(&user_id));
    let confirmed_subscribers = get_confirmed_subscribers(&connection_pool)
        .await?;

    for subscriber in confirmed_subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client.send_email(
                    &subscriber.email, &body.title, &body.content.html, &body.content.text)
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter to {}", &subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(error.cause_chain = ?error, "Skipping a confirmed subscriber. Their stored contact details are invalid.")
            }
        }
    }

    Ok(HttpResponse::Ok().finish())
}

struct Credentials {
    username: String,
    password: Secret<String>,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let authorization_header = headers.get(header::AUTHORIZATION)
        .context("The 'Authorization' was not present in headers.")?
        .to_str()
        .context("The 'Authorization' headers was not a valid UTF-8 string.")?;
    let encoded_credentials = authorization_header
        .strip_prefix("Basic ")
        .context("The 'Authorization' header is not using Basic authentication scheme.")?;
    let decoded_bytes = base64::engine::general_purpose::STANDARD.decode(encoded_credentials)
        .context("Failed to decode base64 encoded 'Basic' credentials.")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("Decoded credentials were not a valid UTF-8 string.")?;
    let mut credentials = decoded_credentials.splitn(2, ":");
    let username = credentials.next()
        .ok_or_else(|| {
            anyhow!("A username must be provided in 'Basic' credentials.")
        })?
        .to_string();
    let password = credentials.next()
        .ok_or_else(|| {
            anyhow!("A password must be provided in 'Basic' credentials.")
        })?
        .to_string();
    
    Ok(Credentials { username, password: Secret::new(password) })
}

#[tracing::instrument(
    name = "Validate credentials",
    skip(credentials, pool),
)]
async fn validate_credentials(credentials: Credentials, pool: &PgPool) -> Result<uuid::Uuid, PublishError> {
    let mut user_id = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$8IKX9HPbtAcLjsMNmh3VAw$jAMZZuiHZyqOYYbmbvPzepp6q1zds5nlLsgwnwCXANQ".to_string()
    );
    if let Some((stored_user_id, stored_password_hash)) = get_stored_credentials(&credentials.username, pool)
        .await
        .map_err(PublishError::UnexpectedError)?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    }

    spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")
    .map_err(PublishError::UnexpectedError)??;

    user_id.ok_or_else(||
        PublishError::AuthError(anyhow!("Unknown username."))
    )
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password)
)]
fn verify_password_hash(expected_password_hash: Secret<String>, password: Secret<String>) -> Result<(), PublishError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC string format.")
        .map_err(PublishError::UnexpectedError)?;

    Argon2::default()
        .verify_password(
            password.expose_secret().as_bytes(),
            &expected_password_hash
        )
        .context("Invalid password.")
        .map_err(PublishError::AuthError)
}

#[tracing::instrument(
    name = "Get stored credentials",
    skip(username, pool),
)]
async fn get_stored_credentials(username: &str, pool: &PgPool) -> Result<Option<(Uuid, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"
            SELECT user_id, password_hash
            FROM t_users
            WHERE username = $1
        "#,
        username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform query to validate user credentials.")?
    .map(|row| (row.user_id, Secret::new(row.password_hash)));

    Ok(row)
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(
    name = "Get confirmed subscribers",
    skip(pool),
)]
pub async fn get_confirmed_subscribers(pool: &PgPool) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let rows = sqlx::query!(
        r#"
            SELECT email
            FROM t_subscriptions
            WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await
    .with_context(|| {
        "A database error was encountered while trying to fetch confirmed subscribers"
    })?;

    let confirmed_subscribers = rows.into_iter()
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error))
        })
        .collect();

    Ok(confirmed_subscribers)
}