use crate::{AuthError, oauth::config::OAuthProvider};

const KEY_PREFIX: &str = "oauth2:csrf:";
const TTL_SECONDS: u64 = 600;

fn validate_state(provider: OAuthProvider, stored_provider: Option<&str>) -> Result<(), AuthError> {
    match stored_provider {
        Some(stored) if stored == provider.as_str() => Ok(()),
        _ => Err(AuthError::InvalidOAuthState),
    }
}

pub async fn store_oauth_state(
    redis: &redis::aio::ConnectionManager,
    provider: &str,
    state: &str,
) -> Result<(), AuthError> {
    let provider = OAuthProvider::parse(provider)?;
    let mut connection = redis.clone();
    let _: () = redis::AsyncCommands::set_ex(
        &mut connection,
        format!("{KEY_PREFIX}{state}"),
        provider.as_str(),
        TTL_SECONDS,
    )
    .await?;
    Ok(())
}

pub async fn consume_oauth_state(
    redis: &redis::aio::ConnectionManager,
    provider: &str,
    state: &str,
) -> Result<(), AuthError> {
    let provider = OAuthProvider::parse(provider)?;
    let mut connection = redis.clone();
    let stored_provider: Option<String> =
        redis::AsyncCommands::get_del(&mut connection, format!("{KEY_PREFIX}{state}")).await?;
    validate_state(provider, stored_provider.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_state_requires_matching_provider_and_nonempty_stored_value() {
        let provider = OAuthProvider::Google;
        assert!(validate_state(provider, Some("google")).is_ok());
        assert!(matches!(
            validate_state(provider, Some("github")),
            Err(AuthError::InvalidOAuthState)
        ));
        assert!(matches!(
            validate_state(provider, None),
            Err(AuthError::InvalidOAuthState)
        ));
    }
}
