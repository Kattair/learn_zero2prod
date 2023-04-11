use actix_web::{
    web::{self, Form},
    HttpResponse,
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[tracing::instrument(
    name = "Adding new subscriber",
    skip(form, connection_pool),
    fields(
        subscriber_name = %form.name,
        subscriber_email = %form.email))]
pub async fn subscribe(form: Form<FormData>, connection_pool: web::Data<PgPool>) -> HttpResponse {
    match insert_subscriber(&form, &connection_pool).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[tracing::instrument(
    name = "Persisting new subscriber details in database",
    skip(form, connection_pool)
)]
pub async fn insert_subscriber(
    form: &FormData,
    connection_pool: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
            INSERT INTO t_subscriptions (id, email, name, subscribed_at)
            VALUES ($1, $2, $3, $4)
            "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now().naive_utc()
    )
    .execute(connection_pool)
    .await
    .map_err(|e| {
        // careful with GDPR, this logs email when unique constraint errors
        tracing::error!("Failed to persist new subscriber: {:?}", e);
        e
    })?;

    Ok(())
}
