use actix_web::{http::header::ContentType, web, HttpResponse};
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use anyhow::Context;
use sqlx::PgPool;

use std::fmt::Write;

use crate::{
    authentication::UserId,
    domain::SubscriberEmail,
    email_client::EmailClient,
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
    skip(user_id, body, connection_pool, email_client),
    fields(user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    user_id: web::ReqData<UserId>,
    body: web::Form<BodyData>,
    connection_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, actix_web::Error> {
    let success_message = FlashMessage::info("The newsletter issues has been published!");
    let user_id = user_id.into_inner();
    tracing::Span::current().record("user_id", tracing::field::display(&user_id));

    let BodyData {
        title,
        plaintext,
        html,
        idempotency_key,
    } = body.0;

    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(utils::e400)?;
    let transaction =
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

    let confirmed_subscribers = get_confirmed_subscribers(&connection_pool)
        .await
        .map_err(utils::e500)?;

    for subscriber in confirmed_subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(&subscriber.email, &title, &html, &plaintext)
                    .await
                    .with_context(|| format!("Failed to send newsletter to {}", &subscriber.email))
                    .map_err(utils::e500)?;
            }
            Err(error) => {
                tracing::warn!(error.cause_chain = ?error, "Skipping a confirmed subscriber. Their stored contact details are invalid.")
            }
        }
    }

    success_message.send();
    let response = see_other("/admin/newsletters");
    let response = idempotency::save_response(transaction, &idempotency_key, &user_id, response)
        .await
        .map_err(utils::e500)?;
    Ok(response)
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
pub async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
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

    let confirmed_subscribers = rows
        .into_iter()
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(confirmed_subscribers)
}
