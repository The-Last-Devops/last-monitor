use super::*;

#[derive(Serialize)]
pub struct ChannelRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub config: Value,
}

/// GET /api/namespaces/:id/channels
pub async fn list_channels(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
) -> Result<Json<Vec<ChannelRow>>, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Viewer).await?;
    let rows: Vec<(Uuid, String, String, sqlx::types::Json<Value>)> = sqlx::query_as(
        "SELECT id, name, kind, config FROM channels WHERE namespace_id = $1 ORDER BY name",
    )
    .bind(ns)
    .fetch_all(&state.config)
    .await
    .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, name, kind, config)| ChannelRow {
                id,
                name,
                kind,
                config: config.0,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct CreateChannel {
    pub name: String,
    pub kind: String, // webhook | telegram | slack | discord
    #[serde(default)]
    pub config: Option<Value>,
}

const CHANNEL_KINDS: &[&str] = &["webhook", "telegram", "slack", "discord"];

/// POST /api/namespaces/:id/channels — editors+ add a notification channel.
pub async fn create_channel(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
    Json(req): Json<CreateChannel>,
) -> Result<Json<Uuid>, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Editor).await?;
    if !CHANNEL_KINDS.contains(&req.kind.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO channels (namespace_id, name, kind, config) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(ns)
    .bind(&req.name)
    .bind(&req.kind)
    .bind(sqlx::types::Json(
        req.config.unwrap_or_else(|| serde_json::json!({})),
    ))
    .fetch_one(&state.config)
    .await
    .map_err(internal)?;
    Ok(Json(id))
}

#[derive(Deserialize)]
pub struct PatchChannel {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub config: Option<Value>,
}

/// PATCH /api/channels/:id — edit a channel's name / config (editors+).
pub async fn patch_channel(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<PatchChannel>,
) -> Result<StatusCode, StatusCode> {
    let ns = ns_of(
        &state,
        "SELECT namespace_id FROM channels WHERE id = $1",
        id,
    )
    .await?;
    rbac::require_role(&state, &user, ns, Role::Editor).await?;
    sqlx::query(
        "UPDATE channels SET name = COALESCE($2, name), config = COALESCE($3, config) WHERE id = $1",
    )
    .bind(id)
    .bind(req.name)
    .bind(req.config.map(sqlx::types::Json))
    .execute(&state.config)
    .await
    .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/channels/:id/test — send a test notification through the channel.
pub async fn test_channel(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let ns = ns_of(
        &state,
        "SELECT namespace_id FROM channels WHERE id = $1",
        id,
    )
    .await
    .map_err(|s| (s, "not found".into()))?;
    rbac::require_role(&state, &user, ns, Role::Editor)
        .await
        .map_err(|s| (s, "forbidden".into()))?;
    let row: Option<(String, sqlx::types::Json<Value>)> =
        sqlx::query_as("SELECT kind, config FROM channels WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.config)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (kind, config) = row.ok_or((StatusCode::NOT_FOUND, "not found".into()))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::alert::dispatch(
        &client,
        &kind,
        &config.0,
        "✅ Test notification from Last Monitor — this channel works.",
    )
    .await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- alert rules ------------------------------------------------------------

#[derive(Serialize)]
pub struct AlertRow {
    pub id: Uuid,
    pub monitor_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub channel_id: Uuid,
    pub cooldown_secs: i32,
    pub enabled: bool,
    pub condition: Value,
}

/// GET /api/namespaces/:id/alerts — rules whose target lives in this namespace.
pub async fn list_alerts(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
) -> Result<Json<Vec<AlertRow>>, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Viewer).await?;
    let rows: Vec<(Uuid, Option<Uuid>, Option<Uuid>, Uuid, i32, bool, sqlx::types::Json<Value>)> =
        sqlx::query_as(
            "SELECT r.id, r.monitor_id, r.system_id, r.channel_id, r.cooldown_secs, r.enabled, r.condition \
             FROM alerts r \
             LEFT JOIN monitors m ON m.id = r.monitor_id \
             LEFT JOIN systems s ON s.id = r.system_id \
             WHERE COALESCE(m.namespace_id, s.namespace_id) = $1",
        )
        .bind(ns)
        .fetch_all(&state.config)
        .await
        .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(id, monitor_id, system_id, channel_id, cooldown_secs, enabled, condition)| {
                    AlertRow {
                        id,
                        monitor_id,
                        system_id,
                        channel_id,
                        cooldown_secs,
                        enabled,
                        condition: condition.0,
                    }
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct CreateAlert {
    #[serde(default)]
    pub monitor_id: Option<Uuid>,
    #[serde(default)]
    pub system_id: Option<Uuid>,
    pub channel_id: Uuid,
    #[serde(default)]
    pub condition: Option<Value>,
    #[serde(default)]
    pub cooldown_secs: Option<i32>,
}

/// POST /api/namespaces/:id/alerts — editors+ create an alert rule.
pub async fn create_alert(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
    Json(req): Json<CreateAlert>,
) -> Result<Json<Uuid>, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Editor).await?;
    if req.monitor_id.is_none() && req.system_id.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    // The channel must belong to this namespace.
    let ok: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM channels WHERE id = $1 AND namespace_id = $2")
            .bind(req.channel_id)
            .bind(ns)
            .fetch_optional(&state.config)
            .await
            .map_err(internal)?;
    if ok.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO alerts (monitor_id, system_id, channel_id, condition, cooldown_secs) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(req.monitor_id)
    .bind(req.system_id)
    .bind(req.channel_id)
    .bind(sqlx::types::Json(
        req.condition.unwrap_or_else(|| serde_json::json!({})),
    ))
    .bind(req.cooldown_secs.unwrap_or(300).max(0))
    .fetch_one(&state.config)
    .await
    .map_err(internal)?;
    Ok(Json(id))
}

// ---- edit / delete ----------------------------------------------------------

/// DELETE /api/channels/:id
pub async fn delete_channel(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let ns = ns_of(
        &state,
        "SELECT namespace_id FROM channels WHERE id = $1",
        id,
    )
    .await?;
    rbac::require_role(&state, &user, ns, Role::Editor).await?;
    sqlx::query("DELETE FROM channels WHERE id = $1")
        .bind(id)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/status-pages/:id
pub async fn delete_status_page(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let ns = ns_of(
        &state,
        "SELECT namespace_id FROM status_pages WHERE id = $1",
        id,
    )
    .await?;
    rbac::require_role(&state, &user, ns, Role::Editor).await?;
    sqlx::query("DELETE FROM status_pages WHERE id = $1")
        .bind(id)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/alerts/:id (namespace resolved via the rule's monitor/server).
pub async fn delete_alert(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let ns = ns_of(
        &state,
        "SELECT COALESCE(m.namespace_id, s.namespace_id) FROM alerts r \
         LEFT JOIN monitors m ON m.id = r.monitor_id \
         LEFT JOIN systems s ON s.id = r.system_id WHERE r.id = $1",
        id,
    )
    .await?;
    rbac::require_role(&state, &user, ns, Role::Editor).await?;
    sqlx::query("DELETE FROM alerts WHERE id = $1")
        .bind(id)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct CreateStatusPage {
    pub slug: String,
    pub title: String,
    /// Optional {"monitor_ids":[...]}; empty => all monitors in the namespace.
    #[serde(default)]
    pub config: Option<Value>,
}

/// POST /api/namespaces/:id/status-pages — editors+ publish a public status page.
pub async fn create_status_page(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
    Json(req): Json<CreateStatusPage>,
) -> Result<Json<Uuid>, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Editor).await?;
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO status_pages (namespace_id, slug, title, config) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(ns)
    .bind(&req.slug)
    .bind(&req.title)
    .bind(sqlx::types::Json(
        req.config.unwrap_or_else(|| serde_json::json!({})),
    ))
    .fetch_one(&state.config)
    .await
    .map_err(internal)?;
    Ok(Json(id))
}
