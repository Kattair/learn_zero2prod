use actix_web::{
    web::{self, Form},
    HttpResponse,
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{domain::{NewSubscriber, SubscriberEmail, SubscriberName}, email_client::EmailClient};

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(form: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(form.name)?;
        let email = SubscriberEmail::parse(form.email)?;

        Ok(NewSubscriber { email, name })
    }
}

#[tracing::instrument(
    name = "Adding new subscriber",
    skip(form, connection_pool, email_client),
    fields(
        subscriber_name = %form.name,
        subscriber_email = %form.email))]
pub async fn subscribe(form: Form<FormData>, connection_pool: web::Data<PgPool>, email_client: web::Data<EmailClient>) -> HttpResponse {
    let new_subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(why) => return HttpResponse::BadRequest().body(why),
    };
    if insert_subscriber(&new_subscriber, &connection_pool).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }
    
    if send_confirmation_email(email_client.as_ref(), &new_subscriber)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
}

#[tracing::instrument(
    name = "Sending confirmation email to a new subcriber",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(email_client: &EmailClient, new_subscriber: &NewSubscriber) -> Result<(), reqwest::Error> {
    let confirmation_link = "https://example.com";
    email_client.send_email(
        &new_subscriber.email,
        "Welcome!",
        &format!(
            r#"<h3>Welcome to our newsletter!<h3>
            <p>Click <a href="{}">here</a> to confirm your subcription.</p>"#,
            confirmation_link
        ),
        &format!(
            r#"Welcome to our newsletter!
            Visit {} to confirm your subscription."#,
            confirmation_link
        ))
    .await
}

#[tracing::instrument(
    name = "Persisting new subscriber details in database",
    skip(new_subscriber, connection_pool)
)]
pub async fn insert_subscriber(
    new_subscriber: &NewSubscriber,
    connection_pool: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
            INSERT INTO t_subscriptions (id, email, name, subscribed_at, status)
            VALUES ($1, $2, $3, $4, 'confirmed')
            "#,
        Uuid::new_v4(),
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
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
