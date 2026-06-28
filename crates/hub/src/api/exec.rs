//! Shell/exec API: per-host shell config, the caller's own SSH credential, and the
//! step-up ticket that opens a console (docs/exec-design.md). Reads redact the key;
//! the private key is only ever accepted (to seal) or unsealed transiently.

use super::*;
use crate::console::ExecTicket;
use crate::exec_crypto;

/// Fetch the bits of a system row the shell endpoints need.
async fn system_shell_row(
    state: &AppState,
    id: Uuid,
) -> Result<(Uuid, Uuid, String, bool, i32), StatusCode> {
    sqlx::query_as::<_, (Uuid, Uuid, String, bool, i32)>(
        "SELECT namespace_id, key_id, hostname, shell_enabled, ssh_port FROM systems WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.config)
    .await
    .map_err(internal)?
    .ok_or(StatusCode::NOT_FOUND)
}

#[derive(Serialize)]
pub struct Credential {
    ssh_user: String,
    key_fingerprint: String,
}

#[derive(Serialize)]
pub struct ShellStatus {
    shell_enabled: bool,
    ssh_port: i32,
    tunnel_online: bool,
    can_exec: bool,
    credential: Option<Credential>,
}

/// GET /api/systems/:id/shell — status for the calling user (any namespace member).
pub async fn get_shell(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ShellStatus>, StatusCode> {
    let (ns, key_id, hostname, shell_enabled, ssh_port) = system_shell_row(&state, id).await?;
    rbac::require_role(&state, &user, ns, Role::Viewer).await?;
    let can_exec = rbac::require_exec(&state, &user, ns).await.is_ok();
    let tunnel_online = state.tunnels.has(key_id, &hostname).await;
    let credential = sqlx::query_as::<_, (String, String)>(
        "SELECT ssh_user, key_fingerprint FROM exec_credentials WHERE user_id = $1 AND system_id = $2",
    )
    .bind(user.id)
    .bind(id)
    .fetch_optional(&state.config)
    .await
    .map_err(internal)?
    .map(|(ssh_user, key_fingerprint)| Credential {
        ssh_user,
        key_fingerprint,
    });
    Ok(Json(ShellStatus {
        shell_enabled,
        ssh_port,
        tunnel_online,
        can_exec,
        credential,
    }))
}

#[derive(Deserialize)]
pub struct PutShell {
    shell_enabled: bool,
    ssh_port: i32,
}

/// PUT /api/systems/:id/shell — owner toggles the host-level shell opt-in + port.
pub async fn put_shell(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<PutShell>,
) -> Result<StatusCode, StatusCode> {
    let (ns, ..) = system_shell_row(&state, id).await?;
    rbac::require_role(&state, &user, ns, Role::Owner).await?;
    if !(1..=65535).contains(&req.ssh_port) {
        return Err(StatusCode::BAD_REQUEST);
    }
    sqlx::query("UPDATE systems SET shell_enabled = $2, ssh_port = $3 WHERE id = $1")
        .bind(id)
        .bind(req.shell_enabled)
        .bind(req.ssh_port)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct PutCred {
    ssh_user: String,
    private_key: String,
    password: String,
}

/// PUT /api/systems/:id/ssh-cred — store the caller's own SSH key for this host,
/// encrypted under their password. Requires exec rights + the correct password (so
/// the key can later be unsealed at step-up with that same password).
pub async fn put_ssh_cred(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<PutCred>,
) -> Result<Json<Credential>, StatusCode> {
    let (ns, ..) = system_shell_row(&state, id).await?;
    rbac::require_exec(&state, &user, ns).await?;

    let ssh_user = req.ssh_user.trim().to_string();
    // Linux usernames: 1–32 chars, no control/space/colon.
    if ssh_user.is_empty()
        || ssh_user.len() > 32
        || ssh_user
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == ':')
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    // The password must be the caller's own login password (the KEK is derived from
    // it; step-up will re-derive with the same password).
    if !crate::auth::verify_user_password(&state.config, user.id, &req.password)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Validate the key parses and derive its fingerprint.
    let key = russh::keys::decode_secret_key(&req.private_key, None)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let fingerprint = key
        .clone_public_key()
        .map(|p| p.fingerprint())
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let salt = exec_crypto::gen_salt();
    let key_enc =
        exec_crypto::seal(&req.password, &salt, req.private_key.as_bytes()).map_err(internal)?;

    sqlx::query(
        "INSERT INTO exec_credentials (user_id, system_id, ssh_user, key_enc, kdf_salt, key_fingerprint) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (user_id, system_id) DO UPDATE SET \
            ssh_user = $3, key_enc = $4, kdf_salt = $5, key_fingerprint = $6",
    )
    .bind(user.id)
    .bind(id)
    .bind(&ssh_user)
    .bind(&key_enc)
    .bind(&salt)
    .bind(&fingerprint)
    .execute(&state.config)
    .await
    .map_err(internal)?;

    Ok(Json(Credential {
        ssh_user,
        key_fingerprint: fingerprint,
    }))
}

/// DELETE /api/systems/:id/ssh-cred — remove the caller's credential.
pub async fn delete_ssh_cred(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let (ns, ..) = system_shell_row(&state, id).await?;
    rbac::require_exec(&state, &user, ns).await?;
    sqlx::query("DELETE FROM exec_credentials WHERE user_id = $1 AND system_id = $2")
        .bind(user.id)
        .bind(id)
        .execute(&state.config)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct TicketReq {
    password: String,
}

#[derive(Serialize)]
pub struct TicketResp {
    ticket: String,
}

/// POST /api/systems/:id/console/ticket — step-up: verify password, unseal the key,
/// mint a single-use console ticket. 400s describe the blocker.
pub async fn console_ticket(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<TicketReq>,
) -> Result<Json<TicketResp>, StatusCode> {
    let (ns, key_id, hostname, shell_enabled, ssh_port) = system_shell_row(&state, id).await?;
    rbac::require_exec(&state, &user, ns).await?;
    if !shell_enabled {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !state.tunnels.has(key_id, &hostname).await {
        return Err(StatusCode::BAD_REQUEST); // agent offline
    }
    if !crate::auth::verify_user_password(&state.config, user.id, &req.password)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let cred = sqlx::query_as::<_, (String, Vec<u8>, Vec<u8>)>(
        "SELECT ssh_user, key_enc, kdf_salt FROM exec_credentials WHERE user_id = $1 AND system_id = $2",
    )
    .bind(user.id)
    .bind(id)
    .fetch_optional(&state.config)
    .await
    .map_err(internal)?
    .ok_or(StatusCode::BAD_REQUEST)?; // no credential
    let (ssh_user, key_enc, kdf_salt) = cred;
    let key_pem = exec_crypto::open(&req.password, &kdf_salt, &key_enc)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let ticket = state
        .exec_tickets
        .issue(ExecTicket {
            system_id: id,
            user_id: user.id,
            user_email: user.email.clone(),
            ssh_user,
            key_pem,
            key_id,
            hostname,
            ssh_port: ssh_port.clamp(1, 65535) as u16,
            created: std::time::Instant::now(),
        })
        .await;
    Ok(Json(TicketResp { ticket }))
}
