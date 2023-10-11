use unicode_segmentation::UnicodeSegmentation;

const FORBIDDEN_NAME_CHARACTERS: [char; 9] = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];

#[derive(Debug)]
pub struct SubscriberName(String);

impl SubscriberName {
    pub fn parse(s: String) -> Result<SubscriberName, String> {
        let is_empty_or_whitespace = s.trim().is_empty();
        let is_too_long = s.graphemes(true).count() > 256;
        let contains_forbidden_characters =
            s.chars().any(|ch| FORBIDDEN_NAME_CHARACTERS.contains(&ch));

        if is_empty_or_whitespace || is_too_long || contains_forbidden_characters {
            Err(format!("{} is not a valid subscruber name.", s))
        } else {
            Ok(Self(s))
        }
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberName;
    use claim::{assert_err, assert_ok};

    use super::FORBIDDEN_NAME_CHARACTERS;

    #[test]
    fn long_name_is_accepted() {
        let name = "a".repeat(256);
        assert_ok!(SubscriberName::parse(name));
    }

    #[test]
    fn too_long_name_is_rejected() {
        let name = "a".repeat(257);
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn blank_name_is_rejected() {
        let name = " ".to_owned();
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn empty_name_is_rejected() {
        let name = "".to_owned();
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn name_containing_forbidden_character_is_rejected() {
        for ch in &FORBIDDEN_NAME_CHARACTERS {
            let name = ch.to_string();
            assert_err!(SubscriberName::parse(name));
        }
    }

    #[test]
    fn a_valid_name_is_ok() {
        let name = "John Doe".to_owned();
        assert_ok!(SubscriberName::parse(name));
    }
}
