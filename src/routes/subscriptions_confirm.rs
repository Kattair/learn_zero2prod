use actix_web::{web, HttpResponse};

#[derive(serde::Deserialize)]
pub struct QueryParams {
    _subscription_token: String,
}

#[tracing::instrument(name = "Confirming a pending subscription", skip(_token))]
pub async fn confirm_subscription(_token: web::Query<QueryParams>) -> HttpResponse {
    HttpResponse::Ok().finish()
}
