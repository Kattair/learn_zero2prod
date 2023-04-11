use actix_web::{
    web::{self, Form},
    HttpResponse,
};
use chrono::Utc;
use sqlx::PgPool;
use tracing::Instrument;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

pub async fn subscribe(form: Form<FormData>, connection_pool: web::Data<PgPool>) -> HttpResponse {
    let request_id = Uuid::new_v4();
    let request_span = tracing::info_span!(
        "Adding new subscriber",
        %request_id,
        name = %form.name,
        email = %form.email
    );
    let _request_span_guard = request_span.enter();

    let query_span = tracing::info_span!("Persisting new subscriber detail in database");
    match sqlx::query!(
        r#"
            INSERT INTO t_subscriptions (id, email, name, subscribed_at)
            VALUES ($1, $2, $3, $4)
            "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now().naive_utc()
    )
    .execute(connection_pool.get_ref())
    .instrument(query_span)
    .await
    {
        Ok(_) => {
            tracing::info!("request_id {} - New subscriber persisted.", request_id);
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            tracing::error!(
                "request_id {} - Failed to persist new subscriber: {:?}",
                request_id,
                e
            ); // careful with GDPR, this logs email when unique constraint errors
            HttpResponse::InternalServerError().finish()
        }
    }
}
