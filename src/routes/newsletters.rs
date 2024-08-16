use std::fmt;

use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use reqwest::StatusCode;
use sqlx::PgPool;

use crate::error::error_chain_fmt;

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
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl fmt::Debug for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}


pub async fn publish_newsletter(body: web::Json<BodyData>, connection_pool: web::Data<PgPool>) -> Result<HttpResponse, PublishError> {
    let confirmed_subscribers = get_confirmed_subscribers(&connection_pool)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

struct ConfirmedSubscriber {
    email: String,
}

#[tracing::instrument(
    name = "Get confirmed subscribers",
    skip(pool),
)]
pub async fn get_confirmed_subscribers(pool: &PgPool) -> Result<Vec<ConfirmedSubscriber>, anyhow::Error> {
    let rows = sqlx::query_as!(
        ConfirmedSubscriber,
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

    Ok(rows)
}