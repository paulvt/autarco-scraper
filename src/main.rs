use std::path::Path;
use std::sync::Mutex;

use color_eyre::Result;
use lazy_static::lazy_static;
use rocket::serde::json::Json;
use rocket::tokio::fs::File;
use rocket::tokio::io::AsyncReadExt;
use rocket::tokio::select;
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

/// The configuration for the My Autarco site
#[derive(Debug, Deserialize)]
struct Config {
    /// The username of the account to login with
    username: String,
    /// The password of the account to login with
    password: String,
    /// The Autarco site ID to track
    site_id: String,
}

/// Loads the configuration.
///
/// The configuration file `autarco.toml` should be located in the project path.
///
/// # Errors
///
/// Returns an error if the file could not be found, opened or read and if the contents are
/// not valid TOML or does not contain all the necessary keys (see [`Config`]).
async fn load_config() -> Result<Config> {
    let config_file_name = Path::new(env!("CARGO_MANIFEST_DIR")).join("autarco.toml");
    let mut file = File::open(config_file_name).await?;

    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    let config = toml::from_str(&contents)?;

    Ok(config)
}

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

lazy_static! {
    /// The concurrently accessible current status.
    static ref STATUS: Mutex<Option<Status>> = Mutex::new(None);
}

/// Returns the current (last known) status.
#[get("/", format = "application/json")]
async fn status() -> Option<Json<Status>> {
    let status_guard = STATUS.lock().expect("Status mutex was poisoined");
    status_guard.map(Json)
}

/// Starts the main update loop and sets up and launches Rocket.
#[rocket::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let rocket = rocket::build().mount("/", routes![status]).ignite().await?;
    let shutdown = rocket.shutdown();

    let updater = rocket::tokio::spawn(update_loop());

    select! {
        result = rocket.launch() => {
            result?;
        },
        result = updater => {
            shutdown.notify();
            result??;
        }
    }

    Ok(())
}
