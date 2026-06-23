use super::*;

/// GET /api/admin/data — DB size, per-table size/rows, retention tiers.
pub async fn data_stats(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<crate::data_admin::DataStats>, StatusCode> {
    if !user.is_admin {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(Json(crate::data_admin::stats(&state.data).await))
}

#[derive(Deserialize)]
pub struct SetRetention {
    pub table: String,
    /// Interpreted in the tier's unit (hours for raw, days for rollups).
    pub value: i64,
}

/// POST /api/admin/retention — change a tier's retention window.
pub async fn set_retention(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<SetRetention>,
) -> Result<StatusCode, StatusCode> {
    if !user.is_admin {
        return Err(StatusCode::FORBIDDEN);
    }
    crate::data_admin::set_retention(&state.data, &req.table, req.value)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- namespaces -------------------------------------------------------------
