use std::fmt;

use validator::validate_email;

#[derive(Debug, serde::Serialize)]
pub struct SubscriberEmail(String);

impl SubscriberEmail {
    pub fn parse(s: String) -> Result<SubscriberEmail, String> {
        if validate_email(&s) {
            Ok(Self(s))
        } else {
            Err(format!("{} is not a valid email address.", s))
        }
    }
}

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SubscriberEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::SubscriberEmail;
    use claim::assert_err;
    use fake::{faker, Fake};

    #[derive(Clone, Debug)]
    struct ValidEmailFixture(pub String);

    impl quickcheck::Arbitrary for ValidEmailFixture {
        fn arbitrary(_: &mut quickcheck::Gen) -> Self {
            // it would be better to call fake_with_rng(), but quickcheck crate
            // removed that possibility as of version 1.0.0
            let email = faker::internet::en::SafeEmail().fake();
            Self(email)
        }
    }

    #[test]
    fn empty_string_is_rejected() {
        let email = "".to_owned();
        assert_err!(SubscriberEmail::parse(email));
    }

    #[test]
    fn blank_string_is_rejected() {
        let email = " ".to_owned();
        assert_err!(SubscriberEmail::parse(email));
    }

    #[test]
    fn email_missing_at_is_rejected() {
        let email = "johnemail.com".to_owned();
        assert_err!(SubscriberEmail::parse(email));
    }

    #[test]
    fn email_missing_subject_is_rejected() {
        let email = "@email.com".to_owned();
        assert_err!(SubscriberEmail::parse(email));
    }

    #[quickcheck_macros::quickcheck]
    fn valid_emails_are_parsed_successfully(valid_email: ValidEmailFixture) -> bool {
        SubscriberEmail::parse(valid_email.0).is_ok()
    }
}
