use serde::Deserialize;

use super::OAuthUserInfo;
use crate::{AuthError, oauth::http::get_json};

const PROFILE_URL: &str = "https://api.github.com/user";
const EMAILS_URL: &str = "https://api.github.com/user/emails";

#[derive(Deserialize)]
struct Profile {
    id: u64,
    login: String,
    email: Option<String>,
    avatar_url: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct Email {
    email: String,
    primary: bool,
}

pub(super) async fn user_info(access_token: &str) -> Result<OAuthUserInfo, AuthError> {
    let profile: Profile = get_json(PROFILE_URL, access_token, true).await?;
    let email = match profile.email.as_ref() {
        Some(email) => Some(email.clone()),
        None => primary_email(get_json(EMAILS_URL, access_token, true).await?),
    };

    Ok(map_profile(profile, email))
}

fn map_profile(profile: Profile, email: Option<String>) -> OAuthUserInfo {
    OAuthUserInfo {
        provider_user_id: profile.id.to_string(),
        email,
        nickname: profile.name.unwrap_or(profile.login),
        avatar_url: profile.avatar_url,
    }
}

fn primary_email(emails: Vec<Email>) -> Option<String> {
    emails
        .into_iter()
        .find(|email| email.primary)
        .map(|email| email.email)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_github_profile_and_uses_primary_email() {
        let profile: Profile = serde_json::from_str(
            r#"{"id":42,"login":"github-user","email":null,"avatar_url":"https://example.com/avatar","name":"GitHub User"}"#,
        )
        .unwrap();
        let emails: Vec<Email> = serde_json::from_str(
            r#"[{"email":"secondary@example.com","primary":false},{"email":"primary@example.com","primary":true}]"#,
        )
        .unwrap();

        let user = map_profile(profile, primary_email(emails));
        assert_eq!(user.provider_user_id, "42");
        assert_eq!(user.nickname, "GitHub User");
        assert_eq!(user.email.as_deref(), Some("primary@example.com"));
        assert_eq!(
            user.avatar_url.as_deref(),
            Some("https://example.com/avatar")
        );
    }

    #[test]
    fn github_profile_falls_back_to_login_and_missing_primary_email() {
        let profile: Profile =
            serde_json::from_str(r#"{"id":7,"login":"fallback","email":null}"#).unwrap();
        let emails: Vec<Email> =
            serde_json::from_str(r#"[{"email":"other@example.com","primary":false}]"#).unwrap();

        let user = map_profile(profile, primary_email(emails));
        assert_eq!(user.nickname, "fallback");
        assert_eq!(user.email, None);
        assert_eq!(user.avatar_url, None);
    }

    #[test]
    fn uses_github_profile_and_email_endpoints() {
        assert_eq!(PROFILE_URL, "https://api.github.com/user");
        assert_eq!(EMAILS_URL, "https://api.github.com/user/emails");
    }
}
