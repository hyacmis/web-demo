use config::{Config, File};
use garde::Validate;
use serde::Deserialize;
use thiserror::Error;

use auth_core::{Oauth2, SecretString};

#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
pub struct App {
    #[garde(custom(crate::helper::validation::validate_config_host))]
    pub host: String,
    #[garde(range(min = 1))]
    pub port: u16,
    #[garde(custom(crate::helper::validation::validate_config_environment))]
    pub env: String,
}

impl App {
    pub fn is_prod(&self) -> bool {
        self.env == "prod"
    }
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
pub struct Log {
    #[garde(custom(crate::helper::validation::validate_log_level))]
    pub level: String,
    #[garde(custom(crate::helper::validation::validate_log_path))]
    pub path: String,
    #[garde(custom(crate::helper::validation::validate_log_filename))]
    pub filename: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
pub struct Database {
    pub debug: bool,
    #[garde(custom(crate::helper::validation::validate_database_url))]
    pub url: SecretString,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct Jwt {
    #[garde(custom(crate::helper::validation::validate_jwt_secret))]
    pub secret: SecretString,
    #[garde(range(min = 1, max = 876_000))]
    pub expires_in_hours: u64,
    #[garde(custom(crate::helper::validation::validate_refresh_secret(&self.secret)))]
    pub refresh_secret: SecretString,
    #[garde(range(min = 1, max = 36_500))]
    pub refresh_expires_in_days: u64,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
pub struct Redis {
    #[garde(custom(crate::helper::validation::validate_config_host))]
    pub host: String,
    #[garde(range(min = 1))]
    pub port: u16,
    pub password: Option<SecretString>,
    pub db: Option<u16>,
}

impl Redis {
    pub fn url(&self) -> String {
        let host = if self.host.parse::<std::net::Ipv6Addr>().is_ok() {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let auth = self.password.as_ref().map_or_else(String::new, |password| {
            let encoded = percent_encoding::utf8_percent_encode(
                password.expose_secret(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string();
            format!(":{}@", encoded)
        });
        let db = self.db.map_or_else(String::new, |db| format!("/{}", db));

        format!("redis://{}{}:{}{}", auth, host, self.port, db)
    }
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct Configs {
    #[garde(dive)]
    pub app: App,
    #[garde(dive)]
    pub log: Log,
    #[garde(dive)]
    pub database: Database,
    #[garde(dive)]
    pub jwt: Jwt,
    #[garde(dive)]
    pub redis: Redis,
    #[serde(default)]
    #[garde(dive)]
    pub oauth2: Oauth2,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to load config.toml")]
    LoadFile,
    #[error("config.toml has an invalid structure")]
    ParseFile,
    #[error("configuration validation failed: {0}")]
    Validation(#[from] garde::Report),
}

impl Configs {
    pub fn load() -> Result<Self, ConfigError> {
        let configs = Config::builder()
            .add_source(File::with_name("config.toml").required(false))
            .build()
            .map_err(|_| ConfigError::LoadFile)?
            .try_deserialize::<Self>()
            .map_err(|_| ConfigError::ParseFile)?;
        Validate::validate(&configs).map_err(ConfigError::Validation)?;
        Ok(configs)
    }
}

#[cfg(test)]
mod tests {
    use super::{Configs, Jwt, Redis};
    use config::{Config, File, FileFormat};
    use garde::Validate;
    use serde_json::Value;

    fn deserialize_and_validate(value: Value) -> Result<Configs, String> {
        let configs: Configs = serde_json::from_value(value).map_err(|error| error.to_string())?;
        Validate::validate(&configs).map_err(|error| error.to_string())?;
        Ok(configs)
    }

    fn parse_toml(contents: &str) -> Result<Configs, config::ConfigError> {
        Config::builder()
            .add_source(File::from_str(contents, FileFormat::Toml))
            .build()
            .unwrap()
            .try_deserialize()
    }

    #[test]
    fn toml_loads_all_configuration_including_secrets() {
        let config = parse_toml(
            r#"
                [app]
                host = "127.0.0.1"
                port = 8000
                env = "dev"
                [log]
                level = "info"
                path = "logs"
                filename = "app.log"
                [database]
                debug = false
                url = "sqlite://data.db?mode=rwc"
                [jwt]
                secret = "toml-access-secret-that-is-long-enough"
                expires_in_hours = 1
                refresh_secret = "toml-refresh-secret-that-is-long-enough"
                refresh_expires_in_days = 7
                [redis]
                host = "localhost"
                port = 6379
                password = "toml-redis-password"
                [oauth2.providers.github]
                client_id = "toml-client-id"
                client_secret = "toml-client-secret"
                auth_url = "https://github.com/login/oauth/authorize"
                token_url = "https://github.com/login/oauth/access_token"
                redirect_url = "https://example.com/callback"
                scopes = ["user:email"]
            "#,
        )
        .unwrap();
        Validate::validate(&config).unwrap();

        assert_eq!(
            config.database.url.expose_secret(),
            "sqlite://data.db?mode=rwc"
        );
        assert_eq!(
            config.jwt.secret.expose_secret(),
            "toml-access-secret-that-is-long-enough"
        );
        assert_eq!(
            config.jwt.refresh_secret.expose_secret(),
            "toml-refresh-secret-that-is-long-enough"
        );
        assert_eq!(
            config
                .redis
                .password
                .as_ref()
                .map(|value| value.expose_secret()),
            Some("toml-redis-password")
        );
        let github = &config.oauth2.providers["github"];
        assert_eq!(github.client_id.expose_secret(), "toml-client-id");
        assert_eq!(github.client_secret.expose_secret(), "toml-client-secret");
    }

    #[test]
    fn example_toml_deserializes_but_requires_configuration() {
        let config = parse_toml(include_str!("../../config.toml.example")).unwrap();

        assert!(Validate::validate(&config).is_err());
    }

    #[test]
    fn redis_url_supports_optional_auth_database_and_ipv6() {
        let redis = Redis {
            host: "2001:db8::1".to_string(),
            port: 6379,
            password: Some("p@ss:word".into()),
            db: Some(2),
        };

        assert_eq!(redis.url(), "redis://:p%40ss%3Aword@[2001:db8::1]:6379/2");
    }

    #[test]
    fn redis_url_omits_absent_auth_and_database() {
        let redis = Redis {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            db: None,
        };

        assert_eq!(redis.url(), "redis://localhost:6379");
    }

    #[test]
    fn serde_deserialization_rejects_invalid_jwt_configuration() {
        let jwt = serde_json::from_str::<Jwt>(
            r#"{
                "secret": "short",
                "expires_in_hours": 1,
                "refresh_secret": "also-short",
                "refresh_expires_in_days": 7
            }"#,
        )
        .expect("JWT configuration should deserialize before validation");

        assert!(jwt.validate().is_err());
    }

    #[test]
    fn config_deserialization_validates_nested_sections() {
        let valid_config = serde_json::json!({
            "app": { "host": "127.0.0.1", "port": 8000, "env": "dev" },
            "log": { "level": "info", "path": "logs", "filename": "app.log" },
            "database": { "debug": false, "url": "sqlite://data.db?mode=rwc" },
            "jwt": {
                "secret": "a-valid-secret-with-at-least-32-bytes",
                "expires_in_hours": 1,
                "refresh_secret": "a-different-valid-secret-of-32-bytes",
                "refresh_expires_in_days": 7
            },
            "redis": { "host": "localhost", "port": 6379 },
            "oauth2": {
                "providers": {
                    "github": {
                        "client_id": "",
                        "client_secret": "",
                        "auth_url": "https://github.com/login/oauth/authorize",
                        "token_url": "https://github.com/login/oauth/access_token",
                        "redirect_url": "http://localhost:8000/callback",
                        "scopes": ["user:email"]
                    }
                }
            }
        });

        assert!(deserialize_and_validate(valid_config.clone()).is_ok());

        let mut invalid_config = valid_config.clone();
        invalid_config["redis"]["port"] = serde_json::json!(0);
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config.clone();
        invalid_config["log"]["level"] = serde_json::json!("verbose");
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config.clone();
        invalid_config["app"]["env"] = serde_json::json!("production");
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config.clone();
        invalid_config["log"]["filename"] = serde_json::json!("logs/app.log");
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config.clone();
        invalid_config["database"]["url"] = serde_json::json!("file://data.db");
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config.clone();
        invalid_config["database"]["url"] = serde_json::json!("mysql:///app");
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config.clone();
        invalid_config["jwt"]["refresh_secret"] = invalid_config["jwt"]["secret"].clone();
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config.clone();
        invalid_config["jwt"]["expires_in_hours"] = serde_json::json!(0);
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config.clone();
        invalid_config["oauth2"]["providers"]["github"]["client_secret"] =
            serde_json::json!("configured-without-client-id");
        assert!(deserialize_and_validate(invalid_config).is_err());

        let mut invalid_config = valid_config;
        invalid_config["oauth2"]["providers"]["github"]["redirect_url"] =
            serde_json::json!("not-a-url");
        assert!(deserialize_and_validate(invalid_config).is_err());
    }
}
