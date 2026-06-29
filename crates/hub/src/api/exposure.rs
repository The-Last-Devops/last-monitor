//! Public-exposure self-check. The hub fetches its own configured public URL at a
//! marker path that lives OUTSIDE `/pub/*`. If that request comes back 200 with the
//! marker, the hub is reachable from the internet with no auth gate in front — we then
//! advise putting it behind nginx basic-auth / Cloudflare Zero Trust (allowing `/pub/*`
//! through for agents). A gate (Access/basic-auth) returns 302/401/403 for the
//! gate-less request → "protected". See docs/exposure.md.

use super::*;
use std::time::Duration;

/// Marker returned by the unauthenticated, non-`/pub` probe endpoint.
const MARKER: &str = "vantage-exposure-ok";

/// GET /exposure-check — unauthenticated, returns a constant marker. Used only by the
/// exposure self-check; carries no data. NOT under `/pub`, so a correctly configured
/// gate (which only bypasses `/pub`) blocks it from the outside.
pub async fn exposure_marker() -> &'static str {
    MARKER
}

/// The hub's public base URL (env `PUBLIC_URL`, else the first `WEBAUTHN_ORIGIN`).
fn public_url() -> Option<String> {
    let pick = |v: String| {
        v.split(',')
            .next()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
    };
    std::env::var("PUBLIC_URL")
        .ok()
        .and_then(pick)
        .or_else(|| std::env::var("WEBAUTHN_ORIGIN").ok().and_then(pick))
}

#[derive(Serialize)]
pub struct ExposureResult {
    configured: bool,
    public_url: Option<String>,
    exposed: Option<bool>, // None when we couldn't determine it
    status: Option<u16>,
    error: Option<String>,
}

/// POST /api/admin/exposure-check — probe our own public URL and report whether the
/// app is reachable without an auth gate. Admin-only; makes one short outbound request.
pub async fn exposure_check(
    State(_state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ExposureResult>, StatusCode> {
    if !user.is_admin {
        return Err(StatusCode::FORBIDDEN);
    }
    let Some(base) = public_url() else {
        return Ok(Json(ExposureResult {
            configured: false,
            public_url: None,
            exposed: None,
            status: None,
            error: None,
        }));
    };
    let url = format!("{base}/exposure-check");
    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none()) // a gate's login redirect = protected
        .timeout(Duration::from_secs(6))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Ok(Json(ExposureResult {
                configured: true,
                public_url: Some(base),
                exposed: None,
                status: None,
                error: Some(e.to_string()),
            }))
        }
    };
    match client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            // Exposed only if the gate-less request actually reached our marker.
            let exposed = status == 200 && body.contains(MARKER);
            Ok(Json(ExposureResult {
                configured: true,
                public_url: Some(base),
                exposed: Some(exposed),
                status: Some(status),
                error: None,
            }))
        }
        Err(e) => Ok(Json(ExposureResult {
            configured: true,
            public_url: Some(base),
            exposed: None,
            status: None,
            error: Some(e.to_string()),
        })),
    }
}
