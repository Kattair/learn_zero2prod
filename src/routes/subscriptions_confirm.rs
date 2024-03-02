use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;

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
) -> HttpResponse {
    let token = query_params.subscription_token.as_ref();
    let subscriber_id = match get_subscriber_id_by_token(&connection_pool, token).await {
        Ok(subscriber_id) => match subscriber_id {
            Some(subscriber_id) => subscriber_id,
            None => return HttpResponse::InternalServerError().finish(),
        },
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    if confirm_subscriber(&connection_pool, subscriber_id)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

#[tracing::instrument(name = "Get subscriber id by token", skip(connection_pool, token))]
async fn get_subscriber_id_by_token(
    connection_pool: &PgPool,
    token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT subscriber_id FROM t_subscription_tokens WHERE subscription_token = $1",
        token
    )
    .fetch_optional(connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get subscriber id by token: {:?}", e);
        e
    })?;

    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(name = "Confirming subscriber", skip(connection_pool))]
async fn confirm_subscriber(connection_pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE t_subscriptions SET status = 'confirmed' WHERE id = $1",
        id
    )
    .execute(connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to confirm subscriber: {:?}", e);
        e
    })?;
    Ok(())
}
