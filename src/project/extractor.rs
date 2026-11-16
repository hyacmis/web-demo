use axum::{
    body::Body,
    extract::{FromRequest, FromRequestParts, Json, Path, Query},
    http::{Request, StatusCode, request::Parts},
};
use garde::Validate;
use serde::de::DeserializeOwned;

use super::error::AppError;

/// 路径参数 ID 提取器（u32，保证非负）
#[allow(dead_code)]
pub struct ResourceId(pub u32);

#[allow(dead_code)]
impl ResourceId {
    /// 安全转换为 i32（与 Entity 主键类型匹配），超出范围返回 400
    pub fn as_i32(&self) -> Result<i32, AppError> {
        i32::try_from(self.0)
            .map_err(|_| AppError::Api(StatusCode::BAD_REQUEST, "ID 超出范围".to_string()))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for ResourceId {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(id): Path<u32> = Path::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::Api(StatusCode::BAD_REQUEST, "无效的ID".to_string()))?;

        Ok(ResourceId(id))
    }
}

/// 自动校验的 JSON 提取器
///
/// 在反序列化完成后自动调用 Garde 校验，校验失败返回 400。
/// 用法完全等同于 `Json<T>`：
///
/// ```rust,ignore
/// async fn handler(ValidatedJson(input): ValidatedJson<RegisterInput>) { .. }
/// ```
pub struct ValidatedJson<T>(pub T);

impl<T> std::ops::Deref for ValidatedJson<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, S> FromRequest<S, Body> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    T::Context: Default,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let value = Json::<T>::from_request(req, state)
            .await
            .map_err(|_| AppError::Api(StatusCode::BAD_REQUEST, "请求参数格式错误".to_string()))?;
        value.validate().map_err(AppError::Validation)?;
        Ok(ValidatedJson(value.0))
    }
}

/// 自动校验的 Query 提取器
///
/// 在反序列化完成后自动调用 `validate()`，校验失败返回 400。
///
/// ```rust,ignore
/// async fn handler(ValidatedQuery(input): ValidatedQuery<UserListInput>) { .. }
/// ```
pub struct ValidatedQuery<T>(pub T);

impl<T> std::ops::Deref for ValidatedQuery<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Validate,
    T::Context: Default,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let value = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|e| AppError::Api(StatusCode::BAD_REQUEST, e.body_text()))?;
        value.validate().map_err(AppError::Validation)?;
        Ok(ValidatedQuery(value.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::FromRequest;
    use axum::http::Request;
    use serde::Deserialize;

    #[test]
    fn resource_id_as_i32_within_range() {
        let id = ResourceId(42);
        assert_eq!(id.as_i32().unwrap(), 42i32);
    }

    #[test]
    fn resource_id_as_i32_max_i32() {
        let id = ResourceId(i32::MAX as u32);
        assert_eq!(id.as_i32().unwrap(), i32::MAX);
    }

    #[test]
    fn resource_id_as_i32_out_of_range() {
        let id = ResourceId(i32::MAX as u32 + 1);
        let err = id.as_i32().unwrap_err();
        match err {
            AppError::Api(StatusCode::BAD_REQUEST, ref msg) => {
                assert_eq!(msg, "ID 超出范围");
            }
            _ => panic!("期望 Api(400, ..)"),
        }
    }

    #[test]
    fn validated_json_deref() {
        let wrapper = ValidatedJson(42i32);
        assert_eq!(*wrapper, 42);
    }

    #[test]
    fn validated_query_deref() {
        let wrapper = ValidatedQuery("hello".to_string());
        assert_eq!(*wrapper, "hello");
    }

    // ── ValidatedJson 提取路径测试 ──

    #[derive(Debug, Deserialize, Validate)]
    #[garde(allow_unvalidated)]
    struct TestInput {
        #[garde(length(min = 1))]
        name: String,
        #[garde(skip)]
        age: u32,
    }

    #[tokio::test]
    async fn validated_json_success() {
        let req = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"Alice","age":30}"#))
            .unwrap();
        let ValidatedJson(input) = ValidatedJson::<TestInput>::from_request(req, &())
            .await
            .unwrap();
        assert_eq!(input.name, "Alice");
        assert_eq!(input.age, 30);
    }

    #[tokio::test]
    async fn validated_json_invalid_body() {
        let req = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from("NOT JSON"))
            .unwrap();
        let err = ValidatedJson::<TestInput>::from_request(req, &())
            .await
            .err()
            .unwrap();
        match err {
            AppError::Api(StatusCode::BAD_REQUEST, ref msg) => {
                assert_eq!(msg, "请求参数格式错误");
            }
            other => panic!("期望 Api(400, ..), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn validated_json_validation_error() {
        let req = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"","age":30}"#))
            .unwrap();
        let err = ValidatedJson::<TestInput>::from_request(req, &())
            .await
            .err()
            .unwrap();
        assert!(
            matches!(err, AppError::Validation(_)),
            "期望 Validation, got {:?}",
            err
        );
    }

    // ── ValidatedQuery 提取路径测试 ──

    #[derive(Debug, Deserialize, Validate)]
    #[garde(allow_unvalidated)]
    struct TestQuery {
        #[garde(length(min = 1))]
        keyword: String,
        #[garde(skip)]
        page: Option<u32>,
    }

    fn build_parts(uri: &str) -> Parts {
        Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap()
            .into_parts()
            .0
    }

    #[tokio::test]
    async fn validated_query_success() {
        let mut parts = build_parts("/search?keyword=hello&page=2");
        let ValidatedQuery(input) =
            ValidatedQuery::<TestQuery>::from_request_parts(&mut parts, &())
                .await
                .unwrap();
        assert_eq!(input.keyword, "hello");
        assert_eq!(input.page, Some(2));
    }

    #[tokio::test]
    async fn validated_query_missing_required() {
        let mut parts = build_parts("/search?page=1");
        let err = ValidatedQuery::<TestQuery>::from_request_parts(&mut parts, &())
            .await
            .err()
            .unwrap();
        match err {
            AppError::Api(StatusCode::BAD_REQUEST, _) => {}
            other => panic!("期望 Api(400, ..), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn validated_query_validation_error() {
        let mut parts = build_parts("/search?keyword=");
        let err = ValidatedQuery::<TestQuery>::from_request_parts(&mut parts, &())
            .await
            .err()
            .unwrap();
        assert!(
            matches!(err, AppError::Validation(_)),
            "期望 Validation, got {:?}",
            err
        );
    }
}
