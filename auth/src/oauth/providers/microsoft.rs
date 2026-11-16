use crate::AuthError;

use super::{OAuthUserInfo, openid};

const USER_INFO_URL: &str = "https://graph.microsoft.com/oidc/userinfo";

pub(super) async fn user_info(access_token: &str) -> Result<OAuthUserInfo, AuthError> {
    openid::user_info(USER_INFO_URL, access_token, "microsoft").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_microsoft_graph_openid_endpoint() {
        assert_eq!(USER_INFO_URL, "https://graph.microsoft.com/oidc/userinfo");
    }
}
