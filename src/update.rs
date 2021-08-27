//! Module for handling the status updating/retrieval via the My Autarco site/API.

use std::time::{Duration, SystemTime};

use reqwest::{Client, ClientBuilder, Error, StatusCode};
use rocket::tokio::time::sleep;
use serde::Deserialize;
use url::{ParseError, Url};

use super::{load_config, Config, Status, BASE_URL, POLL_INTERVAL, STATUS};

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

/// The energy data returned by the energy API endpoint.
#[derive(Debug, Deserialize)]
struct ApiEnergy {
    /// Total energy produced today (kWh)
    pv_today: u32,
    /// Total energy produced this month (kWh)
    pv_month: u32,
    /// Total energy produced since installation (kWh)
    pv_to_date: u32,
}

///  The power data returned by the power API endpoint.
#[derive(Debug, Deserialize)]
struct ApiPower {
    /// Current power production (W)
    pv_now: u32,
}

/// Performs a login on the My Autarco site.
///
/// It mainly stores the acquired cookie in the client's cookie jar. The login credentials come
/// from the loaded configuration (see [`Config`]).
async fn login(config: &Config, client: &Client) -> Result<(), Error> {
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
async fn update(config: &Config, client: &Client, last_updated: u64) -> Result<Status, Error> {
    // Retrieve the data from the API endpoints.
    let api_energy_url = api_url(&config.site_id, "energy").expect("valid API energy URL");
    let api_response = client.get(api_energy_url).send().await?;
    let api_energy: ApiEnergy = match api_response.error_for_status() {
        Ok(res) => res.json().await?,
        Err(err) => return Err(err),
    };

    let api_power_url = api_url(&config.site_id, "power").expect("valid API power URL");
    let api_response = client.get(api_power_url).send().await?;
    let api_power: ApiPower = match api_response.error_for_status() {
        Ok(res) => res.json().await?,
        Err(err) => return Err(err),
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
pub(super) async fn update_loop() -> color_eyre::Result<()> {
    let config = load_config().await?;
    let client = ClientBuilder::new().cookie_store(true).build()?;

    // Go to the My Autarco site and login.
    println!("⚡ Logging in...");
    login(&config, &client).await?;
    println!("⚡ Logged in successfully!");

    let mut last_updated = 0;
    loop {
        // Wake up every 10 seconds and check if an update is due.
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
