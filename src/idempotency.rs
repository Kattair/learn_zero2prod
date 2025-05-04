use std::convert::TryFrom;

use actix_web::{body::to_bytes, HttpResponse};
use reqwest::StatusCode;
use sqlx::{postgres::PgHasArrayType, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug)]
pub struct IdempotencyKey(String);

impl TryFrom<String> for IdempotencyKey {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            anyhow::bail!("IdempotencyKey cannot be empty");
        }

        let max_length = 50;
        if value.len() > max_length {
            anyhow::bail!("IdempotencyKey must be shorter than {max_length} characters");
        }

        Ok(Self(value))
    }
}

impl AsRef<str> for IdempotencyKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<IdempotencyKey> for String {
    fn from(key: IdempotencyKey) -> Self {
        key.0
    }
}

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
pub struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

impl PgHasArrayType for HeaderPairRecord {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_header_pair")
    }
}

pub async fn get_saved_response(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: &Uuid,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    let saved_response = sqlx::query!(
        r#"
        SELECT
            response_status_code as "response_status_code!",
            response_headers as "response_headers!: Vec<HeaderPairRecord>",
            response_body as "response_body!"
        FROM t_idempotency
        WHERE user_id = $1
            AND idempotency_key = $2
        "#,
        user_id,
        idempotency_key.as_ref(),
    )
    .fetch_optional(pool)
    .await?;

    if let Some(r) = saved_response {
        let status_code = StatusCode::from_u16(r.response_status_code.try_into()?)?;
        let mut response = HttpResponse::build(status_code);
        for HeaderPairRecord { name, value } in r.response_headers {
            response.append_header((name, value));
        }
        Ok(Some(response.body(r.response_body)))
    } else {
        Ok(None)
    }
}

pub async fn save_response(
    mut transaction: Transaction<'static, Postgres>,
    idempotency_key: &IdempotencyKey,
    user_id: &Uuid,
    http_response: HttpResponse,
) -> Result<HttpResponse, anyhow::Error> {
    let (response_head, body) = http_response.into_parts();
    let body = to_bytes(body).await.map_err(|e| anyhow::anyhow!("{}", e))?;

    let status_code = response_head.status().as_u16() as i16;
    let headers = {
        let mut h = Vec::with_capacity(response_head.headers().len());
        for (name, value) in response_head.headers() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();
            h.push(HeaderPairRecord { name, value });
        }
        h
    };

    sqlx::query_unchecked!(
        r#"
        UPDATE t_idempotency
        SET response_status_code = $1,
            response_headers = $2,
            response_body = $3
        WHERE user_id = $4
            AND idempotency_key = $5
        "#,
        status_code,
        headers,
        body.as_ref(),
        user_id,
        idempotency_key.as_ref(),
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(response_head.set_body(body).map_into_boxed_body())
}

pub enum NextAction {
    StartProcessing(Transaction<'static, Postgres>),
    ReturnSavedResponse(HttpResponse),
}

pub async fn try_processing(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: &Uuid,
) -> Result<NextAction, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let count_of_inserted_rows = sqlx::query!(
        r#"
        INSERT INTO t_idempotency (
            user_id,
            idempotency_key,
            created_at
        )
        VALUES ($1, $2, now())
        ON CONFLICT DO NOTHING
    "#,
        user_id,
        idempotency_key.as_ref()
    )
    .execute(&mut *transaction)
    .await?
    .rows_affected();

    if count_of_inserted_rows > 0 {
        Ok(NextAction::StartProcessing(transaction))
    } else {
        let saved_response = get_saved_response(pool, idempotency_key, user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Expected to find a saved response"))?;
        Ok(NextAction::ReturnSavedResponse(saved_response))
    }
}
