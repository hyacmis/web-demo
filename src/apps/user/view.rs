use super::constants::ClassType;
use super::schemas::{UserCreate, UserListInput, UserListOutput, UserOutput, UserPatch};
use crate::constants::AppState;
use crate::project::error::{ApiResult, AppError, ok};
use crate::project::extractor::ValidatedJson;
use crate::project::extractor::ValidatedQuery;
use crate::project::middlewares::auth::AuthContext;
use crate::project::pagination::{PagePagination, PaginationInput};
use axum::extract::State;
use axum::http::StatusCode;
use entity::{
    account::{ActiveModel as AccountActiveModel, Column as AccountColumn},
    prelude::Account,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, IntoActiveModel, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

/// 获取用户列表
#[utoipa::path(
    get,
    path = "/user/list",
    tag = "用户管理",
    params(UserListInput,PaginationInput),
    responses(
        (status = 200, body = PagePagination<UserListOutput>),
        (status = 403, description = "仅内部员工可调用")
    )
)]
pub async fn user_list(
    State(state): State<AppState>,
    auth: AuthContext,
    ValidatedQuery(input): ValidatedQuery<UserListInput>,
    ValidatedQuery(pagination): ValidatedQuery<PaginationInput>,
) -> ApiResult<PagePagination<UserListOutput>> {
    require_internal_staff(&auth)?;

    let page = pagination.page;
    let page_size = pagination.page_size;
    let offset = pagination.offset().ok_or_else(|| {
        AppError::Api(
            StatusCode::BAD_REQUEST,
            "分页偏移量超出支持范围".to_string(),
        )
    })?;

    let query = Account::find()
        .select_only()
        .columns([
            AccountColumn::Id,
            AccountColumn::Nickname,
            AccountColumn::Type,
            AccountColumn::Name,
            AccountColumn::Gender,
            AccountColumn::Telephone,
        ])
        .filter(
            Condition::all()
                .add_option(input.r#type.map(|t| AccountColumn::Type.eq(t)))
                .add_option(input.name.map(|n| AccountColumn::Name.contains(n)))
                .add_option(
                    input
                        .telephone
                        .map(|t| AccountColumn::Telephone.contains(t)),
                ),
        );

    let total = query.clone().count(&state.db).await?;
    let data = query
        .order_by_desc(AccountColumn::Id)
        .into_model::<UserListOutput>()
        .paginate(&state.db, page_size)
        .fetch_page(offset / page_size)
        .await?;

    ok(PagePagination {
        data,
        total,
        page,
        page_size,
    })
}

/// 创建用户
#[utoipa::path(
    post,
    path = "/user",
    tag = "用户管理",
    request_body = UserCreate,
    responses(
        (status = 200),
        (status = 400, description = "Validation error"),
        (status = 403, description = "仅内部员工可调用")
    )
)]
pub async fn user_create(
    State(state): State<AppState>,
    auth: AuthContext,
    ValidatedJson(input): ValidatedJson<UserCreate>,
) -> ApiResult<()> {
    require_internal_staff(&auth)?;

    // 密码哈希
    let hashed_password = auth_core::hash_password(&input.password)?;

    let obj = AccountActiveModel {
        uid: Set(Uuid::new_v4().to_string()),
        nickname: Set(input.nickname),
        password: Set(hashed_password),
        name: Set(input.name),
        gender: Set(input.gender.map(|t| t as i16).unwrap_or(0)),
        telephone: Set(input.telephone),
        email: Set(input.email),
        address: Set(input.address),
        r#type: Set(ClassType::User as i16),
        ..Default::default()
    };

    let _obj = obj.insert(&state.db).await?;

    ok(())
}

/// 获取当前用户信息
#[utoipa::path(
    get,
    path = "/user",
    tag = "用户管理",
    responses(
        (status = 200, body = UserOutput),
        (status = 404, description = "User not found")
    )
)]
pub async fn user_detail(
    State(state): State<AppState>,
    auth: AuthContext,
) -> ApiResult<UserOutput> {
    let user = Account::find_by_id(auth.user_id)
        .into_model::<UserOutput>()
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::Api(StatusCode::NOT_FOUND, "用户不存在".to_string()))?;
    ok(user)
}

/// 更新当前用户信息
#[utoipa::path(
    patch,
    path = "/user",
    tag = "用户管理",
    request_body = UserPatch,
    responses(
        (status = 200),
        (status = 404, description = "User not found")
    )
)]
pub async fn user_patch(
    State(state): State<AppState>,
    auth: AuthContext,
    ValidatedJson(data): ValidatedJson<UserPatch>,
) -> ApiResult<()> {
    if data.is_empty() {
        return Err(AppError::Api(
            StatusCode::BAD_REQUEST,
            "至少需要提供一个要更新的字段".to_string(),
        ));
    }

    let mut obj = Account::find_by_id(auth.user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::Api(StatusCode::NOT_FOUND, "用户不存在".to_string()))?
        .into_active_model();

    if let Some(v) = data.nickname {
        obj.nickname = Set(v)
    }
    if let Some(v) = data.password {
        let hashed_password = auth_core::hash_password(&v)?;
        obj.password = Set(hashed_password)
    }
    if let Some(v) = data.gender {
        obj.gender = Set(v as i16)
    }
    if let Some(v) = data.name {
        obj.name = Set(v)
    }
    if let Some(v) = data.telephone {
        obj.telephone = Set(v)
    }
    if let Some(v) = data.email {
        obj.email = Set(v)
    }
    if let Some(v) = data.address {
        obj.address = Set(v)
    }

    obj.update(&state.db).await?;
    ok(())
}

fn require_internal_staff(auth: &AuthContext) -> Result<(), AppError> {
    if auth.user_type != ClassType::InternalStaff as i16 {
        return Err(AppError::Api(
            StatusCode::FORBIDDEN,
            "无权执行用户管理操作".to_string(),
        ));
    }

    Ok(())
}

/// 删除当前用户
#[utoipa::path(
    delete,
    path = "/user",
    tag = "用户管理",
    responses(
        (status = 200),
        (status = 404, description = "User not found")
    )
)]
pub async fn user_delete(State(state): State<AppState>, auth: AuthContext) -> ApiResult<()> {
    let obj = Account::find_by_id(auth.user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::Api(StatusCode::NOT_FOUND, "用户不存在".to_string()))?;
    obj.delete(&state.db).await?;

    ok(())
}
