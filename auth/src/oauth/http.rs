use std::sync::LazyLock;
use std::time::Duration;

use crate::AuthError;

static HTTP_CLIENT: LazyLock<Result<oauth2::reqwest::Client, oauth2::reqwest::Error>> =
    LazyLock::new(|| {
        oauth2::reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .redirect(oauth2::reqwest::redirect::Policy::none())
            .build()
    });

pub(super) fn client() -> Result<&'static oauth2::reqwest::Client, AuthError> {
    HTTP_CLIENT.as_ref().map_err(|_| AuthError::OAuthHttpClient)
}

pub(super) async fn get_json<T>(
    url: &str,
    access_token: &str,
    github_headers: bool,
) -> Result<T, AuthError>
where
    T: serde::de::DeserializeOwned,
{
    let mut request = client()?.get(url).bearer_auth(access_token);
    if github_headers {
        request = request
            .header("User-Agent", "web-demo-auth")
            .header("Accept", "application/vnd.github+json");
    }
    let response = request
        .send()
        .await
        .and_then(oauth2::reqwest::Response::error_for_status)
        .map_err(|_| AuthError::OAuthProfileRequest)?;
    let body = response
        .text()
        .await
        .map_err(|_| AuthError::OAuthProfileResponse)?;
    serde_json::from_str(&body).map_err(|_| AuthError::OAuthProfileResponse)
}
