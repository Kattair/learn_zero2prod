use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use reqwest::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error;

#[derive(thiserror::Error)]
pub enum SubscriptionConfirmError {
    #[error("No matching subscriber found for the provided token.")]
    NotFound,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscriptionConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error::error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscriptionConfirmError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            SubscriptionConfirmError::NotFound => StatusCode::NOT_FOUND,
            SubscriptionConfirmError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(serde::Deserialize)]
pub struct QueryParams {
    subscription_token: String,
}

#[tracing::instrument(
    name = "Confirming a pending subscription",
    skip(query_params, connection_pool)
)]
pub async fn confirm_subscription(
    query_params: web::Query<QueryParams>,
    connection_pool: web::Data<PgPool>,
) -> Result<HttpResponse, SubscriptionConfirmError> {
    let token = query_params.subscription_token.as_ref();
    let subscriber_id = match get_subscriber_id_by_token(&connection_pool, token)
        .await
        .with_context(|| format!("Failed to find matching subscriber for token {token}"))?
    {
        Some(subscriber_id) => subscriber_id,
        None => return Err(SubscriptionConfirmError::NotFound),
    };
    confirm_subscriber(&connection_pool, subscriber_id)
        .await
        .with_context(|| "Failed to confirm subscriber.")?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Get subscriber id by token", skip(connection_pool, token))]
async fn get_subscriber_id_by_token(
    connection_pool: &PgPool,
    token: &str,
) -> Result<Option<Uuid>, anyhow::Error> {
    let result = sqlx::query!(
        "SELECT subscriber_id FROM t_subscription_tokens WHERE subscription_token = $1",
        token
    )
    .fetch_optional(connection_pool)
    .await
    .with_context(|| "A database error was encountered when looking for a matching subscriber.")?;

    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(name = "Confirming subscriber", skip(connection_pool))]
async fn confirm_subscriber(connection_pool: &PgPool, id: Uuid) -> Result<(), anyhow::Error> {
    sqlx::query!(
        "UPDATE t_subscriptions SET status = 'confirmed' WHERE id = $1",
        id
    )
    .execute(connection_pool)
    .await
    .with_context(|| "A database error was encountered when confirming a subscription.")?;

    Ok(())
}
