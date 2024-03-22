use core::fmt;
use std::f32::consts::E;

use actix_web::{
    web::{self, Form},
    HttpResponse, ResponseError,
};
use chrono::Utc;
use rand::distributions::DistString;
use reqwest::StatusCode;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(form: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(form.name)?;
        let email = SubscriberEmail::parse(form.email)?;

        Ok(NewSubscriber { email, name })
    }
}

#[tracing::instrument(
    name = "Adding new subscriber",
    skip(form, connection_pool, email_client, base_url),
    fields(
        subscriber_name = %form.name,
        subscriber_email = %form.email))]
pub async fn subscribe(
    form: Form<FormData>,
    connection_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;
    let mut transaction = connection_pool.begin().await
        .map_err(SubscribeError::PoolError)?;
    let subscriber_id = insert_subscriber(&new_subscriber, &mut transaction).await?;
    let token = generate_subscription_token();
    store_token(&mut transaction, &subscriber_id, &token).await?;
    transaction.commit().await
        .map_err(SubscribeError::TransactionCommitError)?;
    send_confirmation_email(email_client.as_ref(), &new_subscriber, &base_url, &token).await?;

    Ok(HttpResponse::Ok().finish())
}

fn generate_subscription_token() -> String {
    let mut rng = rand::thread_rng();
    rand::distributions::Alphanumeric.sample_string(&mut rng, 48)
}

#[tracing::instrument(
    name = "Sending confirmation email to a new subcriber",
    skip(email_client, new_subscriber, base_url, token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: &NewSubscriber,
    base_url: &ApplicationBaseUrl,
    token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url.0, token
    );
    email_client
        .send_email(
            &new_subscriber.email,
            "Welcome!",
            &format!(
                r#"<h3>Welcome to our newsletter!<h3>
            <p>Click <a href="{}">here</a> to confirm your subcription.</p>"#,
                confirmation_link
            ),
            &format!(
                r#"Welcome to our newsletter!
            Visit {} to confirm your subscription."#,
                confirmation_link
            ),
        )
        .await
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error("Failed to acquire a Postgres connection from the pool.")]
    PoolError(#[source] sqlx::Error),
    #[error("Failed to commit SQL transaction to store a new subscriber.")]
    TransactionCommitError(#[source] sqlx::Error),
    #[error("Failed to store a new subscriber.")]
    InsertSubscriberError(#[from] InsertSubscriberError),
    #[error("Failed to store the confirmation token for a new subscriber.")]
    StoreTokenError(#[from] StoreTokenError),
    #[error("Failed to send a confirmation email.")]
    SendEmailError(#[from] reqwest::Error),
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub struct InsertSubscriberError(sqlx::Error);

impl From<sqlx::Error> for InsertSubscriberError {
    fn from(value: sqlx::Error) -> Self {
        Self(value)
    }
}

impl std::error::Error for InsertSubscriberError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl fmt::Display for InsertSubscriberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A database error was encountered while inserting a new subcriber.")
    }
}

impl fmt::Debug for InsertSubscriberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub struct StoreTokenError(sqlx::Error);

impl From<sqlx::Error> for StoreTokenError {
    fn from(value: sqlx::Error) -> Self {
        Self(value)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A database error was encountered while trying to store a subscription token.")
    }
}

impl fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

fn error_chain_fmt(err: &impl std::error::Error, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "{}\n", err)?;
    let mut current = err.source();
    while let Some(source) = current {
        writeln!(f, "Caused by:\n\t{}", source)?;
        current = source.source();
    }
    Ok(())
}

#[tracing::instrument(
    name = "Persisting new subscriber details in database",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    new_subscriber: &NewSubscriber,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, InsertSubscriberError> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
            INSERT INTO t_subscriptions (id, email, name, subscribed_at, status)
            VALUES ($1, $2, $3, $4, 'pending_confirmation')
            "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now().naive_utc()
    )
    // https://stackoverflow.com/questions/64654769/how-to-build-and-commit-multi-query-transaction-in-sqlx
    .execute(&mut **transaction)
    .await
    .map_err(|e| {
        // careful with GDPR, this logs email when unique constraint errors
        tracing::error!("Failed to persist new subscriber: {:?}", e);
        e
    })?;

    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Storing subscription token in database",
    skip(transaction, token)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: &Uuid,
    token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"
        INSERT INTO t_subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)
        "#,
        token,
        subscriber_id
    )
    // https://stackoverflow.com/questions/64654769/how-to-build-and-commit-multi-query-transaction-in-sqlx
    .execute(&mut **transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store subscription token: {:?}", e);
        e
    })?;

    Ok(())
}
