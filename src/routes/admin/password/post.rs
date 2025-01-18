use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

use crate::{
    authentication::{self, validate_credentials, AuthError, Credentials},
    routes::admin::dashboard::get_username,
    session_state::TypedSession,
    utils,
};

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    form: web::Form<FormData>,
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = session.get_user_id().map_err(utils::e500)?;
    if user_id.is_none() {
        return Ok(utils::see_other("/login"));
    }
    let user_id = user_id.unwrap();

    let username = get_username(user_id, &pool).await.map_err(utils::e500)?;

    let credentials = Credentials {
        username: username,
        password: form.current_password.clone(),
    };

    if let Err(e) = validate_credentials(credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect.").send();
                Ok(utils::see_other("/admin/password"))
            }
            AuthError::UnexpectedError(_) => Err(utils::e500(e)),
        };
    }

    let password_length = form.new_password.expose_secret().len();
    if password_length < 8 {
        FlashMessage::error(
            "The new password is too short - its length must be between 8 and 128 characters.",
        )
        .send();
        return Ok(utils::see_other("/admin/password"));
    } else if password_length > 128 {
        FlashMessage::error(
            "The new password is too long - its length must be between 8 and 128 characters.",
        )
        .send();
        return Ok(utils::see_other("/admin/password"));
    }

    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error("You entered two different password - the field values must match.")
            .send();
        return Ok(utils::see_other("/admin/password"));
    }

    authentication::change_password(user_id, form.0.new_password, &pool)
        .await
        .map_err(utils::e500)?;
    FlashMessage::info("Your password has been changed.").send();
    Ok(utils::see_other("/admin/password"))
}
