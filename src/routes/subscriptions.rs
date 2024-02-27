use actix_web::{
    web::{self, Form},
    HttpResponse,
};
use chrono::Utc;
use rand::distributions::DistString;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};

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
    skip(form, connection_pool, email_client, base_url),
    fields(
        subscriber_name = %form.name,
        subscriber_email = %form.email))]
pub async fn subscribe(
    form: Form<FormData>,
    connection_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> HttpResponse {
    let new_subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(why) => return HttpResponse::BadRequest().body(why),
    };
    let subscriber_id = match insert_subscriber(&new_subscriber, &connection_pool).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let token = generate_subscription_token();
    if store_token(&connection_pool, &subscriber_id, &token).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    if send_confirmation_email(email_client.as_ref(), &new_subscriber, &base_url, &token)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
}

fn generate_subscription_token() -> String {
    let mut rng = rand::thread_rng();
    rand::distributions::Alphanumeric.sample_string(&mut rng, 48)
}

#[tracing::instrument(
    name = "Sending confirmation email to a new subcriber",
    skip(email_client, new_subscriber, base_url, token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: &NewSubscriber,
    base_url: &ApplicationBaseUrl,
    token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url.0,
        token
    );
    email_client
        .send_email(
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
            ),
        )
        .await
}

#[tracing::instrument(
    name = "Persisting new subscriber details in database",
    skip(new_subscriber, connection_pool)
)]
pub async fn insert_subscriber(
    new_subscriber: &NewSubscriber,
    connection_pool: &PgPool,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
            INSERT INTO t_subscriptions (id, email, name, subscribed_at, status)
            VALUES ($1, $2, $3, $4, 'pending_confirmation')
            "#,
        subscriber_id,
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

    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Storing subscription token in database",
    skip(connection_pool, token)
)]
pub async fn store_token(connection_pool: &PgPool, subscriber_id: &Uuid, token: &str) -> Result<(), sqlx::Error>{
    sqlx::query!(r#"
        INSERT INTO t_subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)
        "#,
        token,
        subscriber_id
    )
    .execute(connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store subscription token: {:?}", e);
        e
    })?;

    Ok(())
}
