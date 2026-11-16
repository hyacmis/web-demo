use garde::Validate;
use sea_orm::FromQueryResult;
use sea_orm::prelude::DateTime;
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::constants::{ClassType, GenderType};
use crate::helper::tools::{format_date_time, format_option_date_time};

#[derive(Debug, Deserialize, Validate, ToSchema, IntoParams)]
#[garde(allow_unvalidated)]
#[into_params(parameter_in = Query)]
pub struct UserListInput {
    pub name: Option<String>,
    pub telephone: Option<String>,
    pub r#type: Option<ClassType>,
}

#[derive(Clone, Serialize, Deserialize, FromQueryResult, ToSchema)]
pub struct UserListOutput {
    pub id: i32,
    pub nickname: String,
    pub r#type: ClassType,
    pub name: Option<String>,
    pub gender: Option<GenderType>,
    pub telephone: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, FromQueryResult, ToSchema)]
pub struct UserOutput {
    pub id: i32,
    pub uid: String,
    pub nickname: String,
    pub name: Option<String>,
    pub gender: Option<GenderType>,
    pub telephone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub r#type: ClassType,
    #[serde(serialize_with = "format_option_date_time")]
    #[schema(value_type = Option<String>)]
    pub last_login_dt: Option<DateTime>,
    #[serde(serialize_with = "format_date_time")]
    #[schema(value_type = String)]
    pub create_ts: DateTime,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Validate)]
#[garde(allow_unvalidated)]
pub struct UserCreate {
    #[garde(length(min = 1))]
    pub nickname: String,
    #[garde(length(min = 6), custom(auth_core::validate_bcrypt_password))]
    pub password: String,
    pub name: Option<String>,
    pub gender: Option<GenderType>,
    #[garde(inner(custom(crate::helper::validation::validate_phone)))]
    pub telephone: Option<String>,
    #[garde(email)]
    pub email: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Validate)]
#[garde(allow_unvalidated)]
pub struct UserPatch {
    #[garde(length(min = 1))]
    pub nickname: Option<String>,
    #[garde(inner(length(min = 6), custom(auth_core::validate_bcrypt_password)))]
    pub password: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub name: Option<Option<String>>,
    pub gender: Option<GenderType>,
    #[garde(inner(inner(custom(crate::helper::validation::validate_phone))))]
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub telephone: Option<Option<String>>,
    #[garde(inner(inner(email)))]
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub address: Option<Option<String>>,
}

impl UserPatch {
    pub fn is_empty(&self) -> bool {
        self.nickname.is_none()
            && self.password.is_none()
            && self.name.is_none()
            && self.gender.is_none()
            && self.telephone.is_none()
            && self.email.is_none()
            && self.address.is_none()
    }
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::{UserCreate, UserPatch};
    use garde::Validate;

    #[test]
    fn patch_distinguishes_missing_fields_from_null() {
        let patch: UserPatch = serde_json::from_str(r#"{"name":null,"email":"user@example.com"}"#)
            .expect("patch JSON should deserialize");

        assert!(patch.nickname.is_none());
        assert_eq!(patch.name, Some(None));
        assert_eq!(patch.email, Some(Some("user@example.com".to_string())));
        assert!(patch.address.is_none());
        assert!(!patch.is_empty());
    }

    #[test]
    fn patch_without_fields_is_empty() {
        let patch: UserPatch =
            serde_json::from_str("{}").expect("empty patch JSON should deserialize");
        assert!(patch.is_empty());
    }

    #[test]
    fn patch_validates_present_contact_values_but_allows_clearing() {
        let valid: UserPatch =
            serde_json::from_str(r#"{"telephone":null,"email":"user@example.com"}"#)
                .expect("patch JSON should deserialize");
        assert!(valid.validate().is_ok());

        let invalid: UserPatch =
            serde_json::from_str(r#"{"telephone":"invalid","email":"invalid"}"#)
                .expect("patch JSON should deserialize");
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn create_validates_optional_contact_values() {
        let valid: UserCreate = serde_json::from_str(
            r#"{"nickname":"user","password":"password","telephone":"13800138000","email":"user@example.com"}"#,
        )
        .expect("create JSON should deserialize");
        assert!(valid.validate().is_ok());

        let invalid: UserCreate = serde_json::from_str(
            r#"{"nickname":"user","password":"password","telephone":"invalid","email":"invalid"}"#,
        )
        .expect("create JSON should deserialize");
        assert!(invalid.validate().is_err());
    }
}
