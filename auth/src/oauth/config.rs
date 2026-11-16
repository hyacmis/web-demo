use std::collections::HashMap;

use garde::{Error as GardeError, Validate};
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, EndpointNotSet, EndpointSet, RedirectUrl, TokenUrl};
use serde::Deserialize;

use crate::{AuthError, SecretString};

pub(super) type ConfiguredClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    Github,
    Google,
    Microsoft,
}

impl OAuthProvider {
    pub fn parse(name: &str) -> Result<Self, AuthError> {
        match name {
            "github" => Ok(Self::Github),
            "google" => Ok(Self::Google),
            "microsoft" => Ok(Self::Microsoft),
            _ => Err(AuthError::UnsupportedOAuthProvider),
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Google => "google",
            Self::Microsoft => "microsoft",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
pub struct Oauth2Provider {
    #[serde(default)]
    pub client_id: SecretString,
    #[serde(default)]
    #[garde(custom(validate_oauth_credentials(&self.client_id)))]
    pub client_secret: SecretString,
    #[garde(url)]
    pub auth_url: String,
    #[garde(url)]
    pub token_url: String,
    #[garde(url)]
    pub redirect_url: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Validate)]
pub struct Oauth2 {
    #[garde(custom(validate_provider_names))]
    #[garde(dive)]
    pub providers: HashMap<String, Oauth2Provider>,
}

fn validate_oauth_credentials<'a>(
    client_id: &'a SecretString,
) -> impl FnOnce(&SecretString, &()) -> garde::Result + 'a {
    move |client_secret, _| {
        if client_id.expose_secret().is_empty() == client_secret.expose_secret().is_empty() {
            Ok(())
        } else {
            Err(GardeError::new(
                "OAuth client_id and client_secret must both be configured or both be empty",
            ))
        }
    }
}

fn validate_provider_names(providers: &HashMap<String, Oauth2Provider>, _: &()) -> garde::Result {
    if providers
        .keys()
        .all(|provider| matches!(provider.as_str(), "github" | "google" | "microsoft"))
    {
        Ok(())
    } else {
        Err(GardeError::new(
            "OAuth providers must be github, google, or microsoft",
        ))
    }
}

pub(super) fn provider_config<'a>(
    provider: &str,
    providers: &'a HashMap<String, Oauth2Provider>,
) -> Result<(OAuthProvider, &'a Oauth2Provider), AuthError> {
    let provider_kind = OAuthProvider::parse(provider)?;
    let config = providers
        .get(provider_kind.as_str())
        .ok_or(AuthError::OAuthProviderNotConfigured)?;
    if config.client_id.expose_secret().trim().is_empty()
        || config.client_secret.expose_secret().trim().is_empty()
    {
        return Err(AuthError::OAuthProviderNotConfigured);
    }
    Ok((provider_kind, config))
}

pub(super) fn client(config: &Oauth2Provider) -> Result<ConfiguredClient, AuthError> {
    let auth_url = AuthUrl::new(config.auth_url.clone())
        .map_err(|_| AuthError::InvalidOAuthProviderConfiguration)?;
    let token_url = TokenUrl::new(config.token_url.clone())
        .map_err(|_| AuthError::InvalidOAuthProviderConfiguration)?;
    let redirect_url = RedirectUrl::new(config.redirect_url.clone())
        .map_err(|_| AuthError::InvalidOAuthProviderConfiguration)?;

    Ok(
        BasicClient::new(ClientId::new(config.client_id.expose_secret().to_string()))
            .set_client_secret(ClientSecret::new(
                config.client_secret.expose_secret().to_string(),
            ))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url),
    )
}
