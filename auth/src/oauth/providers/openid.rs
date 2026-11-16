use serde::Deserialize;

use super::OAuthUserInfo;
use crate::{AuthError, oauth::http::get_json};

#[derive(Deserialize)]
struct Profile {
    sub: String,
    email: Option<String>,
    name: Option<String>,
    picture: Option<String>,
}

pub(super) async fn user_info(
    url: &str,
    access_token: &str,
    fallback_prefix: &str,
) -> Result<OAuthUserInfo, AuthError> {
    let profile: Profile = get_json(url, access_token, false).await?;
    Ok(map_profile(profile, fallback_prefix))
}

fn map_profile(profile: Profile, fallback_prefix: &str) -> OAuthUserInfo {
    OAuthUserInfo {
        provider_user_id: profile.sub.clone(),
        email: profile.email,
        nickname: profile
            .name
            .unwrap_or_else(|| format!("{fallback_prefix}_{}", profile.sub)),
        avatar_url: profile.picture,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_openid_profile_fields_and_fallback_name() {
        let profile: Profile = serde_json::from_str(
            r#"{"sub":"provider-user","email":"user@example.com","name":"Provider User","picture":"https://example.com/avatar"}"#,
        )
        .unwrap();
        let user = map_profile(profile, "google");
        assert_eq!(user.provider_user_id, "provider-user");
        assert_eq!(user.email.as_deref(), Some("user@example.com"));
        assert_eq!(user.nickname, "Provider User");
        assert_eq!(
            user.avatar_url.as_deref(),
            Some("https://example.com/avatar")
        );

        let profile: Profile = serde_json::from_str(r#"{"sub":"microsoft-user"}"#).unwrap();
        let user = map_profile(profile, "microsoft");
        assert_eq!(user.nickname, "microsoft_microsoft-user");
        assert_eq!(user.email, None);
        assert_eq!(user.avatar_url, None);
    }
}
