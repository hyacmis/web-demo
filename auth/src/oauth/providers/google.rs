use crate::AuthError;

use super::{OAuthUserInfo, openid};

const USER_INFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";

pub(super) async fn user_info(access_token: &str) -> Result<OAuthUserInfo, AuthError> {
    openid::user_info(USER_INFO_URL, access_token, "google").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_google_openid_userinfo_endpoint() {
        assert_eq!(
            USER_INFO_URL,
            "https://openidconnect.googleapis.com/v1/userinfo"
        );
    }
}
