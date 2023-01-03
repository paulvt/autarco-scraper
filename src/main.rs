#![doc = include_str!("../README.md")]
#![warn(
    clippy::all,
    missing_debug_implementations,
    rust_2018_idioms,
    rustdoc::broken_intra_doc_links
)]
#![deny(missing_docs)]

use std::sync::Mutex;

use once_cell::sync::Lazy;
use rocket::fairing::AdHoc;
use rocket::serde::json::Json;
use rocket::{get, routes};
use serde::{Deserialize, Serialize};

use self::update::update_loop;

mod update;

/// The base URL of My Autarco site.
const BASE_URL: &str = "https://my.autarco.com";

/// The interval between data polls.
///
/// This depends on with which interval Autaurco processes new information from the invertor.
const POLL_INTERVAL: u64 = 300;

/// The extra configuration necessary to access the My Autarco site.
#[derive(Debug, Deserialize)]
struct Config {
    /// The username of the account to login with
    username: String,
    /// The password of the account to login with
    password: String,
    /// The Autarco site ID to track
    site_id: String,
}

/// The global, concurrently accessible current status.
static STATUS: Lazy<Mutex<Option<Status>>> = Lazy::new(|| Mutex::new(None));

/// The current photovoltaic invertor status.
#[derive(Clone, Copy, Debug, Serialize)]
struct Status {
    /// Current power production (W)
    current_w: u32,
    /// Total energy produced since installation (kWh)
    total_kwh: u32,
    /// Timestamp of last update
    last_updated: u64,
}

/// Returns the current (last known) status.
#[get("/", format = "application/json")]
async fn status() -> Option<Json<Status>> {
    let status_guard = STATUS.lock().expect("Status mutex was poisoined");
    status_guard.map(Json)
}

/// Creates a Rocket and attaches the config parsing and update loop as fairings.
#[rocket::launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![status])
        .attach(AdHoc::config::<Config>())
        .attach(AdHoc::on_liftoff("Updater", |rocket| {
            Box::pin(async move {
                // We don't care about the join handle nor error results?
                let config = rocket.figment().extract().expect("Invalid configuration");
                let _ = rocket::tokio::spawn(update_loop(config));
            })
        }))
}
