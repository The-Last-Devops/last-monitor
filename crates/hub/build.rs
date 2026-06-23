use std::fs;
use std::path::Path;

// rust-embed (`#[folder = "../../frontend/dist"]` in spa.rs) needs that directory
// to exist at compile time. The CI cargo jobs (fmt/clippy/test/build) don't run
// `npm run build`, and `vite build` deletes the tracked `.gitkeep`, so the folder
// can be absent — which makes the embed macro skip generating `Assets::get`.
// Ensure the folder (with a placeholder) exists so the crate always compiles; the
// real SPA assets are produced by `npm run build` before the Docker image build.
fn main() {
    let dist = Path::new("../../frontend/dist");
    let _ = fs::create_dir_all(dist);
    let keep = dist.join(".gitkeep");
    if !keep.exists() {
        let _ = fs::write(&keep, b"");
    }

    // Build metadata for the About page.
    let git_sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=GIT_SHA={git_sha}");
    let build_date = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%d %H:%M UTC"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    println!("cargo:rerun-if-changed=build.rs");
}
