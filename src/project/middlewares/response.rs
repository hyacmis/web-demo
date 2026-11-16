use axum::{
    extract::Request,
    http::{HeaderMap, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use serde_json::Value;

use crate::project::response::JsonResponse;

pub async fn redirect_response(request: Request, next: Next) -> impl IntoResponse {
    let path = request.uri().path();
    let method = request.method().clone();

    if is_documentation_path(path) || method == Method::HEAD {
        return next.run(request).await.into_response();
    }

    let res = next.run(request).await;
    let (parts, body) = res.into_parts();

    if parts.status.is_informational()
        || parts.status == StatusCode::NO_CONTENT
        || parts.status == StatusCode::NOT_MODIFIED
    {
        return Response::from_parts(parts, body);
    }

    if parts.status.is_redirection() {
        return Response::from_parts(parts, body);
    }

    let bytes = match body.collect().await {
        Ok(body) => body.to_bytes(),
        Err(err) => {
            tracing::error!("响应正文读取失败: {}", err);
            return JsonResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                "服务器内部错误".to_string(),
                None::<Value>,
            )
            .into_response();
        }
    };

    let data = match serde_json::from_slice::<Value>(&bytes) {
        Ok(value) if is_json_response_envelope(&value) => {
            let mut response = Response::new(axum::body::Body::from(bytes));
            *response.status_mut() = parts.status;
            *response.version_mut() = parts.version;
            copy_non_body_headers(&parts.headers, response.headers_mut());
            return response;
        }
        Ok(value) => value,
        Err(_) => Value::String(String::from_utf8_lossy(&bytes).into_owned()),
    };

    let response = if parts.status == StatusCode::OK {
        JsonResponse::success(data)
    } else {
        JsonResponse::new(
            parts.status.as_u16(),
            status_message(parts.status),
            Some(data),
        )
    };

    let mut res = response.into_response();
    *res.status_mut() = parts.status;
    *res.version_mut() = parts.version;
    copy_non_body_headers(&parts.headers, res.headers_mut());
    res
}

fn is_documentation_path(path: &str) -> bool {
    ["/docs", "/scalar", "/openapi.json"].iter().any(|prefix| {
        path == *prefix
            || path
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

fn is_json_response_envelope(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.contains_key("code") && object.contains_key("message"))
}

fn status_message(status: StatusCode) -> String {
    status
        .canonical_reason()
        .unwrap_or("Request failed")
        .to_string()
}

fn copy_non_body_headers(source: &HeaderMap, target: &mut HeaderMap) {
    for (name, value) in source {
        if name != header::CONTENT_LENGTH
            && name != header::CONTENT_TYPE
            && name != header::CONTENT_ENCODING
            && name != header::CONTENT_RANGE
            && name != header::CONTENT_DISPOSITION
            && name != header::ACCEPT_RANGES
            && name != header::ETAG
            && name != header::LAST_MODIFIED
        {
            target.append(name.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_existing_api_envelopes() {
        assert!(is_json_response_envelope(
            &json!({"code": 400, "message": "invalid", "data": null})
        ));
        assert!(!is_json_response_envelope(
            &json!({"user": "alice", "data": null})
        ));
    }

    #[test]
    fn matches_documentation_paths_only_at_segment_boundaries() {
        assert!(is_documentation_path("/docs"));
        assert!(is_documentation_path("/docs/openapi.json"));
        assert!(!is_documentation_path("/docs-private"));
    }

    #[test]
    fn drops_stale_representation_headers_but_preserves_cookies() {
        let mut source = HeaderMap::new();
        source.insert(header::CONTENT_LENGTH, "100".parse().unwrap());
        source.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
        source.insert(header::ETAG, "old".parse().unwrap());
        source.insert(header::SET_COOKIE, "session=value".parse().unwrap());

        let mut target = HeaderMap::new();
        copy_non_body_headers(&source, &mut target);

        assert!(!target.contains_key(header::CONTENT_LENGTH));
        assert!(!target.contains_key(header::CONTENT_TYPE));
        assert!(!target.contains_key(header::ETAG));
        assert_eq!(target[header::SET_COOKIE], "session=value");
    }
}
