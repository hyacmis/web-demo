use std::path::Path;
use std::str::FromStr;
use std::sync::LazyLock;

use garde::Error;
use regex::Regex;
use tracing::Level;

static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^1[3-9]\d{9}$").unwrap());

pub fn validate_phone(phone: &str, _: &()) -> garde::Result {
    if PHONE_REGEX.is_match(phone) {
        Ok(())
    } else {
        Err(Error::new("手机号格式不正确"))
    }
}

pub fn validate_config_host(host: &str, _: &()) -> garde::Result {
    if !host.is_empty() && host.trim() == host && !host.chars().any(char::is_whitespace) {
        Ok(())
    } else {
        Err(Error::new(
            "host must be non-empty and contain no whitespace",
        ))
    }
}

pub fn validate_config_environment(env: &str, _: &()) -> garde::Result {
    match env {
        "dev" | "prod" => Ok(()),
        _ => Err(Error::new("app.env must be either 'dev' or 'prod'")),
    }
}

pub fn validate_log_level(level: &str, _: &()) -> garde::Result {
    Level::from_str(&level.to_ascii_uppercase())
        .map(|_| ())
        .map_err(|_| Error::new("log.level must be trace, debug, info, warn, or error"))
}

pub fn validate_log_path(path: &str, _: &()) -> garde::Result {
    if !path.is_empty() && path.trim() == path {
        Ok(())
    } else {
        Err(Error::new(
            "log.path must not be empty or padded with whitespace",
        ))
    }
}

pub fn validate_log_filename(filename: &str, _: &()) -> garde::Result {
    let path = Path::new(filename);
    if !filename.is_empty()
        && filename.trim() == filename
        && path.file_name().is_some_and(|name| name == filename)
        && filename != "."
        && filename != ".."
    {
        Ok(())
    } else {
        Err(Error::new("log.filename must be a file name, not a path"))
    }
}

pub fn validate_database_url<T: AsRef<str> + ?Sized>(database_url: &T, _: &()) -> garde::Result {
    let parsed = url::Url::parse(database_url.as_ref())
        .map_err(|_| Error::new("database.url must be a valid connection URL"))?;

    match parsed.scheme() {
        "sqlite" if parsed.host_str().is_some() || !parsed.path().is_empty() => Ok(()),
        "mysql" | "postgres" | "postgresql" if parsed.host_str().is_some() => Ok(()),
        _ => Err(Error::new(
            "database.url must contain a valid SQLite path or network database host",
        )),
    }
}

pub fn validate_jwt_secret<T: AsRef<str> + ?Sized>(secret: &T, _: &()) -> garde::Result {
    let secret = secret.as_ref();
    if secret.trim() == secret && secret.len() >= 32 {
        Ok(())
    } else {
        Err(Error::new(
            "JWT secret must be at least 32 bytes without surrounding whitespace",
        ))
    }
}

pub fn validate_refresh_secret<'a, T: AsRef<str> + ?Sized>(
    access_secret: &'a T,
) -> impl FnOnce(&T, &()) -> garde::Result + 'a {
    move |refresh_secret, _| {
        validate_jwt_secret(refresh_secret, &())?;
        if refresh_secret.as_ref() != access_secret.as_ref() {
            Ok(())
        } else {
            Err(Error::new(
                "JWT access and refresh secrets must be different",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use garde::Validate;

    #[test]
    fn validates_phone_values() {
        assert!(validate_phone("13800138000", &()).is_ok());
        assert!(validate_phone("not-a-phone", &()).is_err());
    }

    #[test]
    fn validates_config_hosts_and_environment_names() {
        assert!(validate_config_host("127.0.0.1", &()).is_ok());
        assert!(validate_config_host("localhost", &()).is_ok());
        assert!(validate_config_host("", &()).is_err());
        assert!(validate_config_host(" local host ", &()).is_err());

        assert!(validate_config_environment("dev", &()).is_ok());
        assert!(validate_config_environment("prod", &()).is_ok());
        assert!(validate_config_environment("production", &()).is_err());
    }

    #[test]
    fn validates_log_settings() {
        assert!(validate_log_level("info", &()).is_ok());
        assert!(validate_log_level("DEBUG", &()).is_ok());
        assert!(validate_log_level("verbose", &()).is_err());

        assert!(validate_log_path("logs", &()).is_ok());
        assert!(validate_log_path("", &()).is_err());
        assert!(validate_log_path(" logs ", &()).is_err());

        assert!(validate_log_filename("web-demo.log", &()).is_ok());
        assert!(validate_log_filename("", &()).is_err());
        assert!(validate_log_filename("logs/web-demo.log", &()).is_err());
        assert!(validate_log_filename("..", &()).is_err());
    }

    #[test]
    fn validates_supported_database_connection_urls() {
        assert!(validate_database_url("sqlite://data.db?mode=rwc", &()).is_ok());
        assert!(validate_database_url("sqlite::memory:", &()).is_ok());
        assert!(validate_database_url("mysql://localhost/app", &()).is_ok());
        assert!(validate_database_url("postgres://localhost/app", &()).is_ok());

        assert!(validate_database_url("not a URL", &()).is_err());
        assert!(validate_database_url("file://data.db", &()).is_err());
        assert!(validate_database_url("mysql:///app", &()).is_err());
    }

    #[test]
    fn validates_jwt_secret_strength_and_separation() {
        let access_secret = "a-valid-access-secret-with-32-bytes";
        let refresh_secret = "a-different-refresh-secret-at-least-32";

        assert!(validate_jwt_secret(access_secret, &()).is_ok());
        assert!(validate_jwt_secret("short", &()).is_err());
        assert!(validate_jwt_secret(" a-valid-secret-with-more-than-32-bytes ", &()).is_err());

        assert!(validate_refresh_secret(access_secret)(refresh_secret, &()).is_ok());
        assert!(validate_refresh_secret(access_secret)(access_secret, &()).is_err());
        assert!(validate_refresh_secret(access_secret)("short", &()).is_err());
    }

    #[test]
    fn garde_email_rule_accepts_and_rejects_addresses() {
        #[derive(garde::Validate)]
        struct EmailInput {
            #[garde(email)]
            email: String,
        }

        assert!(
            EmailInput {
                email: "user@example.com".to_string()
            }
            .validate()
            .is_ok()
        );
        assert!(
            EmailInput {
                email: "invalid-email".to_string()
            }
            .validate()
            .is_err()
        );
    }
}
