//! Optional self-update for the `:auto-update` release channel.
//!
//! The hub polls ghcr for the rolling `:auto-update` tag's digest; when it changes
//! (a newer image was pushed) the hub exits, and k8s re-pulls the new image
//! (`imagePullPolicy: Always`). It never downloads or executes a binary — the
//! update is a normal image pull, same trust model as a manual redeploy.
//!
//! Gated HARD: only when the baked channel is `auto`, running under Kubernetes,
//! and not disabled via `AUTO_UPDATE=0`. Otherwise a no-op. The baseline digest is
//! the one present at startup (the image k8s pulled for us), so the hub only exits
//! after a genuinely newer push — no crash-loop.

use std::time::Duration;

const REPO: &str = "the-last-devops/vantage-hub";
const TAG: &str = "auto-update";
const POLL: Duration = Duration::from_secs(300);

pub fn spawn() {
    if env!("VANTAGE_CHANNEL") != "auto"
        || std::env::var("AUTO_UPDATE").as_deref() == Ok("0")
        || std::env::var("KUBERNETES_SERVICE_HOST").is_err()
    {
        return;
    }
    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let baseline = match fetch_digest(&client).await {
            Some(d) => d,
            None => {
                tracing::warn!("self-update: couldn't read initial :auto-update digest — disabled");
                return;
            }
        };
        tracing::info!(%baseline, "self-update: watching ghcr :auto-update");
        let mut tick = tokio::time::interval(POLL);
        tick.tick().await; // consume the immediate first tick
        loop {
            tick.tick().await;
            if let Some(cur) = fetch_digest(&client).await {
                if cur != baseline {
                    tracing::warn!(%baseline, %cur, "newer :auto-update image — exiting so k8s re-pulls");
                    std::process::exit(0);
                }
            }
        }
    });
}

/// Current digest of `ghcr.io/<REPO>:<TAG>` via an anonymous pull token (public).
async fn fetch_digest(client: &reqwest::Client) -> Option<String> {
    let tok: serde_json::Value = client
        .get(format!(
            "https://ghcr.io/token?scope=repository:{REPO}:pull"
        ))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let token = tok.get("token")?.as_str()?;
    let resp = client
        .head(format!("https://ghcr.io/v2/{REPO}/manifests/{TAG}"))
        .bearer_auth(token)
        .header(
            "Accept",
            "application/vnd.oci.image.index.v1+json, \
             application/vnd.docker.distribution.manifest.list.v2+json",
        )
        .send()
        .await
        .ok()?;
    resp.headers()
        .get("docker-content-digest")?
        .to_str()
        .ok()
        .map(String::from)
}
