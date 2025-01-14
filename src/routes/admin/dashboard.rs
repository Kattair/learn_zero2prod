use actix_web::{http::header::ContentType, web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{session_state::TypedSession, utils};

pub async fn admin_dashboard(
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let username = if let Some(user_id) = session.get_user_id().map_err(utils::e500)? {
        get_username(user_id, &pool).await.map_err(utils::e500)?
    } else {
        return Ok(utils::see_other("/login"));
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(include_str!("dashboard.html"), username = username)))
}

#[tracing::instrument(name = "Get username", skip(pool))]
pub async fn get_username(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        select username
        from t_users
        where user_id = $1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve username.")?;

    Ok(row.username)
}
