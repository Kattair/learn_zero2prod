use std::convert::TryFrom;

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
