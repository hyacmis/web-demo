mod account;
mod config;
mod http;
mod providers;
mod state;

use std::collections::HashMap;

use oauth2::{AuthorizationCode, CsrfToken, Scope, TokenResponse};

use crate::AuthError;

pub use account::link_or_create_account;
pub use config::{OAuthProvider, Oauth2, Oauth2Provider};
pub use providers::OAuthUserInfo;
pub use state::{consume_oauth_state, store_oauth_state};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthAuthorization {
    pub url: String,
    pub state: String,
}

pub fn oauth_authorization(
    provider: &str,
    providers: &HashMap<String, Oauth2Provider>,
) -> Result<OAuthAuthorization, AuthError> {
    let (_, config) = config::provider_config(provider, providers)?;
    let (url, state) = config::client(config)?
        .authorize_url(CsrfToken::new_random)
        .add_scopes(config.scopes.iter().cloned().map(Scope::new))
        .url();

    Ok(OAuthAuthorization {
        url: url.into(),
        state: state.secret().clone(),
    })
}

pub async fn exchange_oauth_code(
    provider: &str,
    providers: &HashMap<String, Oauth2Provider>,
    code: &str,
) -> Result<OAuthUserInfo, AuthError> {
    let (provider_kind, provider_config) = config::provider_config(provider, providers)?;
    let access_token = config::client(provider_config)?
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .request_async(http::client()?)
        .await
        .map_err(|_| AuthError::OAuthCodeExchange)?
        .access_token()
        .secret()
        .to_string();

    providers::profile(provider_kind, &access_token).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use garde::Validate;

    fn provider_config(name: &str) -> Oauth2Provider {
        let (auth_url, token_url, scopes) = match name {
            "github" => (
                "https://github.com/login/oauth/authorize",
                "https://github.com/login/oauth/access_token",
                vec!["user:email"],
            ),
            "google" => (
                "https://accounts.google.com/o/oauth2/v2/auth",
                "https://oauth2.googleapis.com/token",
                vec!["openid", "email"],
            ),
            "microsoft" => (
                "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
                "https://login.microsoftonline.com/common/oauth2/v2.0/token",
                vec!["openid", "User.Read"],
            ),
            _ => unreachable!(),
        };
        Oauth2Provider {
            client_id: format!("{name}-client").into(),
            client_secret: format!("{name}-secret").into(),
            auth_url: auth_url.to_string(),
            token_url: token_url.to_string(),
            redirect_url: format!("https://example.com/{name}/callback"),
            scopes: scopes.into_iter().map(str::to_string).collect(),
        }
    }

    #[test]
    fn builds_authorization_for_each_configured_channel() {
        let providers = ["google", "github", "microsoft"]
            .into_iter()
            .map(|name| (name.to_string(), provider_config(name)))
            .collect();

        for provider in ["google", "github", "microsoft"] {
            let authorization = oauth_authorization(provider, &providers).unwrap();
            assert!(authorization.url.contains("state="));
            assert!(!authorization.state.is_empty());
        }
    }

    #[test]
    fn validates_provider_names_and_credentials() {
        let mut config = Oauth2 {
            providers: HashMap::from([(
                "github".to_string(),
                Oauth2Provider {
                    client_secret: String::new().into(),
                    ..provider_config("github")
                },
            )]),
        };
        assert!(config.validate().is_err());
        config.providers.get_mut("github").unwrap().client_secret = "secret".into();
        assert!(config.validate().is_ok());

        config
            .providers
            .insert("unknown".to_string(), provider_config("github"));
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_unknown_and_unconfigured_channels_before_network_access() {
        assert!(matches!(
            oauth_authorization("unknown", &HashMap::new()),
            Err(AuthError::UnsupportedOAuthProvider)
        ));
        assert!(matches!(
            oauth_authorization("google", &HashMap::new()),
            Err(AuthError::OAuthProviderNotConfigured)
        ));
        assert!(matches!(
            futures::executor::block_on(exchange_oauth_code(
                "microsoft",
                &HashMap::new(),
                "authorization-code"
            )),
            Err(AuthError::OAuthProviderNotConfigured)
        ));
    }
}
