use super::*;
use chrono::{Duration, Utc};

#[derive(Serialize)]
pub struct PatRow {
    pub id: Uuid,
    pub name: String,
    /// Leading characters for display; the full token is shown only once, at creation.
    pub prefix: String,
    pub created_at: String,
    pub last_used: Option<String>,
    pub expires_at: Option<String>,
}

/// GET /api/pats — the caller's own access tokens (never the secret value).
pub async fn list_pats(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<PatRow>>, StatusCode> {
    let rows: Vec<(Uuid, String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, name, prefix, created_at::text, last_used::text, expires_at::text \
         FROM api_pats WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&state.config)
    .await
    .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(id, name, prefix, created_at, last_used, expires_at)| PatRow {
                    id,
                    name,
                    prefix,
                    created_at,
                    last_used,
                    expires_at,
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct CreatePat {
    pub name: String,
    #[serde(default)]
    pub expires_in_days: Option<i64>,
}

#[derive(Serialize)]
pub struct CreatedPat {
    pub id: Uuid,
    /// The full token — returned once, never retrievable again.
    pub token: String,
}

/// POST /api/pats — mint a token that acts AS the caller (inherits their RBAC).
pub async fn create_pat(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<CreatePat>,
) -> Result<Json<CreatedPat>, StatusCode> {
    if !super::valid_name(&req.name, 64) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let token = format!(
        "lm_pat_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let prefix = format!("{}…", &token[..14]);
    let expires_at = req
        .expires_in_days
        .filter(|d| *d > 0)
        .map(|d| Utc::now() + Duration::days(d));
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO api_pats (user_id, name, token_hash, prefix, expires_at) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(user.id)
    .bind(req.name.trim())
    .bind(crate::auth::token_hash(&token))
    .bind(&prefix)
    .bind(expires_at)
    .fetch_one(&state.config)
    .await
    .map_err(internal)?;
    Ok(Json(CreatedPat { id, token }))
}

/// DELETE /api/pats/:id — revoke one of the caller's own tokens.
pub async fn delete_pat(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = sqlx::query("DELETE FROM api_pats WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
