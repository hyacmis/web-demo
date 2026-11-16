use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::openapi::{
    self, Content, RefOr,
    schema::{Object, Schema, Type},
};
use utoipa::{Modify, OpenApi};
use utoipauto::utoipauto;

/// JWT Bearer 安全方案注入
struct JwtSecurityAddon;

impl Modify for JwtSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

/// 顶层 API 文档元数据（utoipauto 自动收集 paths + schemas）
#[utoipauto(paths = "./src")]
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Web Demo API",
        description = "A demo API built with Axum and SeaORM",
        version = "1.0.0"
    ),
    servers(
        (url = "/api/v1", description = "API v1")
    ),
    modifiers(&JwtSecurityAddon),
    security(
        ("bearer_auth" = [])
    )
)]
pub struct ApiDoc;

impl ApiDoc {
    /// 生成 OpenAPI 规范，并将所有响应体包装为 { code, message, data }
    pub fn spec() -> openapi::OpenApi {
        wrap_responses(Self::openapi())
    }
}

/// 将所有响应体包装为 JsonResponse { code, message, data }
fn wrap_responses(mut api: openapi::OpenApi) -> openapi::OpenApi {
    for item in api.paths.paths.values_mut() {
        for op in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.options,
            &mut item.head,
            &mut item.trace,
        ]
        .into_iter()
        .flatten()
        {
            for resp in op.responses.responses.values_mut() {
                let RefOr::T(resp) = resp else { continue };
                if resp.content.is_empty() {
                    resp.content.insert(
                        "application/json".to_string(),
                        RefOr::T(Content::builder().schema(Some(wrap_schema(None))).build()),
                    );
                } else {
                    for content in resp.content.values_mut() {
                        if let RefOr::T(content) = content {
                            content.schema = Some(wrap_schema(content.schema.take()));
                        }
                    }
                }
            }
        }
    }
    api
}

/// 构建 { code: integer, message: string, data: <schema|null> }
fn wrap_schema(data: Option<RefOr<Schema>>) -> RefOr<Schema> {
    let data = data.unwrap_or_else(|| {
        RefOr::T(Schema::Object(
            Object::builder().schema_type(Type::Null).build(),
        ))
    });
    RefOr::T(Schema::Object(
        Object::builder()
            .property(
                "code",
                RefOr::T(Schema::Object(
                    Object::builder().schema_type(Type::Integer).build(),
                )),
            )
            .required("code")
            .property(
                "message",
                RefOr::T(Schema::Object(
                    Object::builder().schema_type(Type::String).build(),
                )),
            )
            .required("message")
            .property("data", data)
            .build(),
    ))
}
