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

pub async fn subscribe(form: Form<FormData>, connection_pool: web::Data<PgPool>) -> HttpResponse {
    let request_id = Uuid::new_v4();
    // careful with GDPR, name and email are considered Personal Identifiable Information
    log::info!(
        "request_id {} - Persisting new subscriber into database with name='{}' and email='{}'.",
        request_id,
        form.name,
        form.email
    );
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
    .await
    {
        Ok(_) => {
            log::info!("request_id {} - New subscriber persisted.", request_id);
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            log::error!(
                "request_id {} - Failed to persist new subscriber: {:?}",
                request_id,
                e
            ); // careful with GDPR, this logs email when unique constraint errors
            HttpResponse::InternalServerError().finish()
        }
    }
}
