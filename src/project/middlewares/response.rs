use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use serde_json::Value;

use crate::project::response::JsonResponse;

// 临时屏蔽报错
#[allow(clippy::result_large_err)]
pub async fn redirect_response(
    request: Request,
    next: Next,
) -> Result<impl IntoResponse, Response> {
    let path = request.uri().path();

    // 放行不需要包装的路径
    if path.starts_with("/docs") || path.starts_with("/scalar") || path.starts_with("/openapi.json")
    {
        return Ok(next.run(request).await.into_response());
    }

    let res = next.run(request).await;
    let (parts, body) = res.into_parts();

    // 放行重定向响应（3xx）
    if parts.status.is_redirection() {
        let mut res = (parts.status, body).into_response();
        res.headers_mut().extend(parts.headers);
        return Ok(res);
    }

    let bytes = body
        .collect()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response())?
        .to_bytes();

    // 将byte转为可读文本
    let str = serde_json::from_slice::<Value>(&bytes);
    let data = match str {
        Ok(s) => s,
        Err(_) => Value::from(String::from_utf8_lossy(&bytes).to_string()),
    };
    // 无法在 Response 没有结果值时返回 {}
    let response = if parts.status == StatusCode::OK {
        JsonResponse::success(data)
    } else {
        JsonResponse::new(parts.status.as_u16(), parts.status.to_string(), Some(data))
    };

    //返回 Response
    let mut res = response.into_response();
    res.headers_mut().extend(parts.headers);
    Ok(res)
}
