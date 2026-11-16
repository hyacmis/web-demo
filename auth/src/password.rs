use garde::Error;

pub fn validate_bcrypt_password(password: &str, _: &()) -> garde::Result {
    if password.len() <= 72 {
        Ok(())
    } else {
        Err(Error::new("密码不能超过72个UTF-8字节"))
    }
}

pub fn hash_password(password: &str) -> Result<String, crate::AuthError> {
    validate_bcrypt_password(password, &()).map_err(|_| crate::AuthError::PasswordTooLong)?;
    bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(Into::into)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, crate::AuthError> {
    if validate_bcrypt_password(password, &()).is_err() {
        return Ok(false);
    }

    bcrypt::verify(password, hash).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bcrypt_password_limit_is_measured_in_bytes() {
        assert!(validate_bcrypt_password(&"a".repeat(72), &()).is_ok());
        assert!(validate_bcrypt_password(&"a".repeat(73), &()).is_err());
        assert!(validate_bcrypt_password(&"密".repeat(25), &()).is_err());
    }

    #[test]
    fn hashes_and_verifies_passwords() {
        let hash = hash_password("my_password").unwrap();
        assert!(verify_password("my_password", &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn overlong_password_is_rejected_without_hashing() {
        assert!(hash_password(&"a".repeat(73)).is_err());
        assert!(!verify_password(&"a".repeat(73), "invalid").unwrap());
    }
}
