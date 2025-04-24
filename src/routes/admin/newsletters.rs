use std::fmt::{self, Debug};

use actix_web::{
    http::header::{self, ContentType},
    web, HttpResponse, ResponseError,
};
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use anyhow::Context;
use reqwest::{header::HeaderValue, StatusCode};
use sqlx::PgPool;

use std::fmt::Write;

use crate::{
    authentication::UserId, domain::SubscriberEmail, email_client::EmailClient,
    error::error_chain_fmt, utils::see_other,
};

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    plaintext: String,
    html: String,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl fmt::Debug for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();

                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);

                response
            }
        }
    }
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
            msg_html = msg_html
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
) -> Result<HttpResponse, PublishError> {
    let user_id = user_id.into_inner();
    tracing::Span::current().record("user_id", tracing::field::display(&user_id));
    let confirmed_subscribers = get_confirmed_subscribers(&connection_pool).await?;

    for subscriber in confirmed_subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(&subscriber.email, &body.title, &body.html, &body.plaintext)
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter to {}", &subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(error.cause_chain = ?error, "Skipping a confirmed subscriber. Their stored contact details are invalid.")
            }
        }
    }

    FlashMessage::info("The newsletter issues has been published!").send();
    Ok(see_other("/admin/newsletters"))
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
