use actix_web::{http::header::ContentType, web, HttpResponse};
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use std::fmt::Write;

use crate::{
    authentication::UserId,
    idempotency::{self, IdempotencyKey},
    utils::{self, see_other},
};

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    plaintext: String,
    html: String,
    idempotency_key: String,
}

pub async fn get_newsletter_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    let mut msg_html = String::new();
    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            include_str!("newsletters.html"),
            msg_html = msg_html,
            idempotency_key = uuid::Uuid::new_v4()
        ))
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(user_id, body, connection_pool),
    fields(user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    user_id: web::ReqData<UserId>,
    body: web::Form<BodyData>,
    connection_pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let success_message = FlashMessage::info("The newsletter issues has been accepted!");
    let user_id = user_id.into_inner();
    tracing::Span::current().record("user_id", tracing::field::display(&user_id));

    let BodyData {
        title,
        plaintext,
        html,
        idempotency_key,
    } = body.0;

    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(utils::e400)?;
    let mut transaction =
        match idempotency::try_processing(&connection_pool, &idempotency_key, &user_id)
            .await
            .map_err(utils::e500)?
        {
            idempotency::NextAction::StartProcessing(t) => t,
            idempotency::NextAction::ReturnSavedResponse(saved_response) => {
                success_message.send();
                return Ok(saved_response);
            }
        };

    let issue_id = insert_newsletter_issue(&mut transaction, &title, &plaintext, &html)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(utils::e500)?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(utils::e500)?;

    let response = see_other("/admin/newsletters");
    let response = idempotency::save_response(transaction, &idempotency_key, &user_id, response)
        .await
        .map_err(utils::e500)?;
    success_message.send();

    Ok(response)
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO t_newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_id,
        title,
        text_content,
        html_content,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(newsletter_id)
}

#[tracing::instrument(skip_all)]
pub async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO t_issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email
        )
        SELECT $1, email
        FROM t_subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
}
