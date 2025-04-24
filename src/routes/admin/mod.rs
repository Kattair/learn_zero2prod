mod dashboard;
mod logout;
mod newsletters;
mod password;

pub use dashboard::admin_dashboard;
pub use logout::log_out;
pub use newsletters::{get_newsletter_form, publish_newsletter};
pub use password::{change_password, change_password_form};
