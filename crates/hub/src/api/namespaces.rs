use super::*;

#[derive(Serialize)]
pub struct NamespaceRow {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub system_count: i64,
    pub member_count: i64,
}

/// GET /api/namespaces — namespaces visible to the caller (all for admins),
/// each with its system and member counts for the management view.
pub async fn list_namespaces(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<NamespaceRow>>, StatusCode> {
    let counts = "(SELECT count(*) FROM systems s WHERE s.namespace_id = n.id), \
                  (SELECT count(*) FROM memberships mm WHERE mm.namespace_id = n.id)";
    let rows: Vec<(Uuid, String, Option<String>, i64, i64)> = if user.can_read_all() {
        sqlx::query_as(&format!(
            "SELECT n.id, n.name, m.role::text, {counts} \
             FROM namespaces n \
             LEFT JOIN memberships m ON m.namespace_id = n.id AND m.user_id = $1 \
             ORDER BY n.name",
        ))
        .bind(user.id)
        .fetch_all(&state.config)
        .await
    } else {
        sqlx::query_as(&format!(
            "SELECT n.id, n.name, m.role::text, {counts} \
             FROM namespaces n \
             JOIN memberships m ON m.namespace_id = n.id \
             WHERE m.user_id = $1 ORDER BY n.name",
        ))
        .bind(user.id)
        .fetch_all(&state.config)
        .await
    }
    .map_err(internal)?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, name, role, system_count, member_count)| NamespaceRow {
                    id,
                    name,
                    role: role.unwrap_or_else(|| "admin".into()),
                    system_count,
                    member_count,
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct CreateNamespace {
    pub name: String,
}

/// Validates a k8s-style namespace name: a DNS label, max 63 chars.
pub fn valid_ns_name(name: &str) -> bool {
    let n = name.len();
    if n == 0 || n > 63 {
        return false;
    }
    let bytes = name.as_bytes();
    let edge_ok = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    if !edge_ok(bytes[0]) || !edge_ok(bytes[n - 1]) {
        return false;
    }
    name.bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

#[cfg(test)]
mod tests {
    use super::valid_ns_name;

    #[test]
    fn namespace_names() {
        assert!(valid_ns_name("default"));
        assert!(valid_ns_name("team-a"));
        assert!(valid_ns_name("prod1"));
        assert!(!valid_ns_name("")); // empty
        assert!(!valid_ns_name("Team-A")); // uppercase
        assert!(!valid_ns_name("-lead")); // leading hyphen
        assert!(!valid_ns_name("trail-")); // trailing hyphen
        assert!(!valid_ns_name("a b")); // space
        assert!(!valid_ns_name(&"x".repeat(64))); // too long
    }
}

/// POST /api/namespaces — any authenticated user may create one; creator becomes owner.
pub async fn create_namespace(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<CreateNamespace>,
) -> Result<Json<NamespaceRow>, StatusCode> {
    if !valid_ns_name(&req.name) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut tx = state.config.begin().await.map_err(internal)?;
    let (id,): (Uuid,) = sqlx::query_as("INSERT INTO namespaces (name) VALUES ($1) RETURNING id")
        .bind(&req.name)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
    sqlx::query("INSERT INTO memberships (user_id, namespace_id, role) VALUES ($1, $2, 'owner')")
        .bind(user.id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;

    Ok(Json(NamespaceRow {
        id,
        name: req.name,
        role: "owner".into(),
        system_count: 0,
        member_count: 1,
    }))
}

/// DELETE /api/namespaces/:id — owners (and admins) only. Refuses to delete the
/// 'default' namespace, or any namespace that still has systems attached
/// (avoids cascading away live hosts by accident).
pub async fn delete_namespace(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    rbac::require_role(&state, &user, id, Role::Owner).await?;

    let (name,): (String,) = sqlx::query_as("SELECT name FROM namespaces WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.config)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if name == "default" {
        return Err(StatusCode::FORBIDDEN);
    }

    let (systems,): (i64,) = sqlx::query_as("SELECT count(*) FROM systems WHERE namespace_id = $1")
        .bind(id)
        .fetch_one(&state.config)
        .await
        .map_err(internal)?;
    if systems > 0 {
        return Err(StatusCode::CONFLICT);
    }

    sqlx::query("DELETE FROM namespaces WHERE id = $1")
        .bind(id)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- alert thresholds (per-namespace, for the "Needs attention" view) -------

/// Warn/crit % thresholds per resource. Defaults: warn 80, crit 90.
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Thresholds {
    pub cpu_warn: f64,
    pub cpu_crit: f64,
    pub mem_warn: f64,
    pub mem_crit: f64,
    pub disk_warn: f64,
    pub disk_crit: f64,
    /// Disk I/O utilization (busiest disk % busy).
    #[serde(default = "default_dutil_warn")]
    pub dutil_warn: f64,
    #[serde(default = "default_dutil_crit")]
    pub dutil_crit: f64,
}

fn default_dutil_warn() -> f64 {
    80.0
}

fn default_dutil_crit() -> f64 {
    95.0
}
impl Default for Thresholds {
    fn default() -> Self {
        Self {
            cpu_warn: 80.0,
            cpu_crit: 90.0,
            mem_warn: 80.0,
            mem_crit: 90.0,
            disk_warn: 80.0,
            disk_crit: 90.0,
            dutil_warn: 80.0,
            dutil_crit: 95.0,
        }
    }
}

#[derive(Serialize)]
pub struct NsThresholds {
    pub namespace: String,
    #[serde(flatten)]
    pub t: Thresholds,
}

/// GET /api/thresholds — effective thresholds for every namespace the caller can
/// see (stored override merged onto the defaults). The fleet UI maps these by
/// namespace name to flag abnormal hosts.
pub async fn list_thresholds(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<NsThresholds>>, StatusCode> {
    let rows: Vec<(String, Option<Value>)> = if user.can_read_all() {
        sqlx::query_as("SELECT name, thresholds FROM namespaces ORDER BY name")
            .fetch_all(&state.config)
            .await
    } else {
        sqlx::query_as(
            "SELECT n.name, n.thresholds FROM namespaces n \
             JOIN memberships m ON m.namespace_id = n.id \
             WHERE m.user_id = $1 ORDER BY n.name",
        )
        .bind(user.id)
        .fetch_all(&state.config)
        .await
    }
    .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(namespace, v)| NsThresholds {
                namespace,
                t: v.and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default(),
            })
            .collect(),
    ))
}

/// PUT /api/namespaces/:id/thresholds — editors+ set the namespace's thresholds.
pub async fn set_thresholds(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
    Json(t): Json<Thresholds>,
) -> Result<StatusCode, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Editor).await?;
    for (w, c) in [
        (t.cpu_warn, t.cpu_crit),
        (t.mem_warn, t.mem_crit),
        (t.disk_warn, t.disk_crit),
        (t.dutil_warn, t.dutil_crit),
    ] {
        if !(0.0..=100.0).contains(&w) || !(0.0..=100.0).contains(&c) || w > c {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let v = serde_json::to_value(t).map_err(internal)?;
    sqlx::query("UPDATE namespaces SET thresholds = $1 WHERE id = $2")
        .bind(v)
        .bind(ns)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- members ----------------------------------------------------------------

#[derive(Serialize)]
pub struct MemberRow {
    pub user_id: Uuid,
    pub email: String,
    pub role: String,
}

/// GET /api/namespaces/:id/members — owners (and admins) list namespace members.
pub async fn list_members(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
) -> Result<Json<Vec<MemberRow>>, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Owner).await?;
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT u.id, u.email, m.role::text FROM memberships m \
         JOIN users u ON u.id = m.user_id WHERE m.namespace_id = $1 ORDER BY u.email",
    )
    .bind(ns)
    .fetch_all(&state.config)
    .await
    .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(user_id, email, role)| MemberRow {
                user_id,
                email,
                role,
            })
            .collect(),
    ))
}

#[derive(Serialize)]
pub struct CandidateRow {
    pub id: Uuid,
    pub email: String,
}

/// GET /api/namespaces/:id/member-candidates — users not yet in this namespace,
/// for the "add member" picker. Owners (and admins) only; minimal fields.
pub async fn member_candidates(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
) -> Result<Json<Vec<CandidateRow>>, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Owner).await?;
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT u.id, u.email FROM users u \
         WHERE u.id NOT IN (SELECT user_id FROM memberships WHERE namespace_id = $1) \
         ORDER BY u.email",
    )
    .bind(ns)
    .fetch_all(&state.config)
    .await
    .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, email)| CandidateRow { id, email })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct AddMember {
    pub email: String,
    pub role: String, // viewer | editor | owner
}

/// POST /api/namespaces/:id/members — owners (and admins) manage membership.
pub async fn add_member(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ns): Path<Uuid>,
    Json(req): Json<AddMember>,
) -> Result<StatusCode, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Owner).await?;
    let role = Role::from_db_str(&req.role).ok_or(StatusCode::BAD_REQUEST)?;

    let (target,): (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.config)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;

    sqlx::query(
        "INSERT INTO memberships (user_id, namespace_id, role) VALUES ($1, $2, $3::ns_role) \
         ON CONFLICT (user_id, namespace_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(target)
    .bind(ns)
    .bind(role.as_db())
    .execute(&state.config)
    .await
    .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- API keys (reusable; systems auto-register) -----------------------------

/// DELETE /api/namespaces/:id/members/:user_id (owners only).
pub async fn delete_member(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((ns, target)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Owner).await?;
    sqlx::query("DELETE FROM memberships WHERE namespace_id = $1 AND user_id = $2")
        .bind(ns)
        .bind(target)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SetExec {
    pub can_exec: bool,
}

/// PUT /api/namespaces/:id/members/:user_id/exec — owners (and admins) grant/revoke
/// the shell/exec capability for a member. Only takes effect for `owner` members
/// (see rbac::require_exec). 404 if the member isn't in the namespace.
pub async fn set_member_exec(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((ns, target)): Path<(Uuid, Uuid)>,
    Json(req): Json<SetExec>,
) -> Result<StatusCode, StatusCode> {
    rbac::require_role(&state, &user, ns, Role::Owner).await?;
    let res = sqlx::query(
        "UPDATE memberships SET can_exec = $3 WHERE namespace_id = $1 AND user_id = $2",
    )
    .bind(ns)
    .bind(target)
    .bind(req.can_exec)
    .execute(&state.config)
    .await
    .map_err(internal)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---- status pages -----------------------------------------------------------
