use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use color_eyre::Result;
use lazy_static::lazy_static;
use reqwest::StatusCode;
use rocket::serde::json::Json;
use rocket::tokio::fs::File;
use rocket::tokio::io::AsyncReadExt;
use rocket::tokio::select;
use rocket::tokio::time::sleep;
use rocket::{get, routes};
use serde::{Deserialize, Serialize};
use url::{ParseError, Url};

/// The interval between data polls
///
/// This depends on with which interval Autaurco processes new information from the invertor.
const POLL_INTERVAL: u64 = 300;

/// The base URL of My Autarco site
const BASE_URL: &'static str = "https://my.autarco.com";

/// Returns the login URL for the My Autarco site.
fn login_url() -> Result<Url, ParseError> {
    Url::parse(&format!("{}/auth/login", BASE_URL))
}

/// Returns an API endpoint URL for the given site ID and endpoint of the My Autarco site.
fn api_url(site_id: &str, endpoint: &str) -> Result<Url, ParseError> {
    Url::parse(&format!(
        "{}/api/site/{}/kpis/{}",
        BASE_URL, site_id, endpoint
    ))
}

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

/// Loads the configuration
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
    /// The concurrently accessible current status
    static ref STATUS: Mutex<Option<Status>> = Mutex::new(None);
}

/// The energy data returned by the energy API endpoint
#[derive(Debug, Deserialize)]
struct ApiEnergy {
    /// Total energy produced today (kWh)
    pv_today: u32,
    /// Total energy produced this month (kWh)
    pv_month: u32,
    /// Total energy produced since installation (kWh)
    pv_to_date: u32,
}

///  The power data returned by the power API endpoint
#[derive(Debug, Deserialize)]
struct ApiPower {
    /// Current power production (W)
    pv_now: u32,
}

/// Performs a login on the My Autarco site
///
/// It mainly stores the acquired cookie in the client's cookie jar. The login credentials come
/// from the loaded configuration (see [`Config`]).
async fn login(config: &Config, client: &reqwest::Client) -> Result<(), reqwest::Error> {
    let params = [
        ("username", &config.username),
        ("password", &config.password),
    ];
    let login_url = login_url().expect("valid login URL");

    client.post(login_url).form(&params).send().await?;

    Ok(())
}

/// Retrieves a status update from the API of the My Autarco site.
///
/// It needs the cookie from the login to be able to perform the action. It uses both the `energy`
/// and `power` endpoint to construct the [`Status`] struct.
async fn update(
    config: &Config,
    client: &reqwest::Client,
    last_updated: u64,
) -> Result<Status, reqwest::Error> {
    // Retrieve the data from the API endpoints.
    let api_energy_url = api_url(&config.site_id, "energy").expect("valid API energy URL");
    let api_response = client.get(api_energy_url).send().await?;
    let api_energy: ApiEnergy = match api_response.error_for_status() {
        Ok(res) => res.json().await?,
        Err(err) => return Err(err.into()),
    };

    let api_power_url = api_url(&config.site_id, "power").expect("valid API power URL");
    let api_response = client.get(api_power_url).send().await?;
    let api_power: ApiPower = match api_response.error_for_status() {
        Ok(res) => res.json().await?,
        Err(err) => return Err(err.into()),
    };

    // Update the status.
    Ok(Status {
        current_w: api_power.pv_now,
        total_kwh: api_energy.pv_to_date,
        last_updated,
    })
}

/// Main update loop that logs in and periodically acquires updates from the API.
///
/// It updates the mutex-guarded current update [`Status`] struct which can be retrieved via
/// Rocket.
async fn update_loop() -> Result<()> {
    let config = load_config().await?;
    let client = reqwest::ClientBuilder::new().cookie_store(true).build()?;

    // Go to the My Autarco site and login
    println!("⚡ Logging in...");
    login(&config, &client).await?;
    println!("⚡ Logged in successfully!");

    let mut last_updated = 0;
    loop {
        // Wake up every 10 seconds and check if there is something to do (quit or update).
        sleep(Duration::from_secs(10)).await;

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if timestamp - last_updated < POLL_INTERVAL {
            continue;
        }

        let status = match update(&config, &client, timestamp).await {
            Ok(status) => status,
            Err(e) if e.status() == Some(StatusCode::UNAUTHORIZED) => {
                println!("✨ Update unauthorized, trying to log in again...");
                login(&config, &client).await?;
                println!("⚡ Logged in successfully!");
                continue;
            }
            Err(e) => {
                println!("✨ Failed to update status: {}", e);
                continue;
            }
        };
        last_updated = timestamp;

        println!("⚡ Updated status to: {:#?}", status);
        let mut status_guard = STATUS.lock().expect("Status mutex was poisoned");
        status_guard.replace(status);
    }
}

/// Returns the current (last known) status
#[get("/", format = "application/json")]
async fn status() -> Option<Json<Status>> {
    let status_guard = STATUS.lock().expect("Status mutex was poisoined");
    status_guard.map(|status| Json(status))
}

/// Starts the main update loop and sets up and launches Rocket
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
