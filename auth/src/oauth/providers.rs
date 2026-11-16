mod github;
mod google;
mod microsoft;
mod openid;

use super::config::OAuthProvider;
use crate::AuthError;

#[derive(Debug, Clone)]
pub struct OAuthUserInfo {
    pub provider_user_id: String,
    pub email: Option<String>,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

pub(super) async fn profile(
    provider: OAuthProvider,
    access_token: &str,
) -> Result<OAuthUserInfo, AuthError> {
    match provider {
        OAuthProvider::Github => github::user_info(access_token).await,
        OAuthProvider::Google => google::user_info(access_token).await,
        OAuthProvider::Microsoft => microsoft::user_info(access_token).await,
    }
}
