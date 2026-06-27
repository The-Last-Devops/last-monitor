//! Host/system JSON endpoints feeding the SPA.
//!
//! Split by concern, each re-exported below so `web::list_systems`,
//! `web::fleet`, `web::system_metrics_series`, `web::system_containers`,
//! `web::system_temps`, and `web::system_gpu` paths stay unchanged:
//! - [`list`] — the systems list with each server's latest sample.
//! - [`metrics`] — per-system metric history for charting.
//! - [`fleet`] — NewRelic-style per-host overlay series across all systems.
//! - [`detail`] — per-system container / temperature / GPU history.
//!
//! [`Series`] (a named timeline of optional values) is shared by the fleet and
//! detail endpoints, so it lives here.

use serde::Serialize;

mod detail;
mod fleet;
mod list;
mod metrics;

pub use detail::{system_containers, system_gpu, system_temps};
pub use fleet::fleet;
pub use list::list_systems;
pub use metrics::system_metrics_series;

#[derive(Serialize)]
pub struct Series {
    pub name: String,
    pub data: Vec<Option<f64>>,
}
