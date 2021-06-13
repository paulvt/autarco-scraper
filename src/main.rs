use std::path::Path;
use std::process::Stdio;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime};

use color_eyre::eyre::eyre;
use color_eyre::Result;
use lazy_static::lazy_static;
use rocket::serde::json::Json;
use rocket::tokio::fs::File;
use rocket::tokio::io::AsyncReadExt;
use rocket::tokio::select;
use rocket::tokio::sync::oneshot::Receiver;
use rocket::tokio::time::sleep;
use rocket::{get, routes};
use serde::{Deserialize, Serialize};
use thirtyfour::prelude::*;
use tokio::process::{Child, Command};

/// The port used by the Gecko Driver
const GECKO_DRIVER_PORT: u16 = 4444;

/// The interval between data polls
///
/// This depends on with which interval Autaurco processes new information from the convertor.
const POLL_INTERVAL: u64 = 300;

/// The URL to the My Autarco site
const URL: &'static str = "https://my.autarco.com/";

/// The login configuration
#[derive(Debug, Deserialize)]
struct Config {
    username: String,
    password: String,
}

/// Spawns the gecko driver
///
/// Note that the function blocks and delays at least a second to ensure everything is up and
/// running.
fn spawn_driver(port: u16) -> Result<Child> {
    // This is taken from the webdriver-client crate.
    let child = Command::new("geckodriver")
        .arg("--port")
        .arg(format!("{}", port))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    thread::sleep(Duration::new(1, 500));

    Ok(child)
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Status {
    current_w: u32,
    total_kwh: u32,
    last_updated: u64,
}

async fn load_config() -> Result<Config> {
    let config_file_name = Path::new(env!("CARGO_MANIFEST_DIR")).join("autarco.toml");
    let mut file = File::open(config_file_name).await?;

    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    let config = toml::from_str(&contents)?;

    Ok(config)
}

async fn login(driver: &WebDriver) -> Result<()> {
    let config = load_config().await?;

    driver.get(URL).await?;

    let input = driver.find_element(By::Id("username")).await?;
    input.send_keys(&config.username).await?;
    let input = driver.find_element(By::Id("password")).await?;
    input.send_keys(&config.password).await?;
    let input = driver.find_element(By::Css("button[type=submit]")).await?;
    input.click().await?;

    Ok(())
}

async fn element_value(driver: &WebDriver, by: By<'_>) -> Result<u32> {
    let element = driver.find_element(by).await?;
    let text = element.text().await?;
    let value = text.parse()?;

    Ok(value)
}

lazy_static! {
    static ref STATUS: Mutex<Option<Status>> = Mutex::new(None);
}

async fn update_loop(mut rx: Receiver<()>) -> Result<()> {
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless()?;
    let driver = WebDriver::new(&format!("http://localhost:{}", GECKO_DRIVER_PORT), &caps).await?;

    // Go to the My Autarco site and login
    println!("⚡ Logging in...");
    // FIXME: Just dropping the driver hangs the process?
    if let Err(e) = login(&driver).await {
        driver.quit().await?;
        return Err(e);
    }

    let mut last_updated = 0;
    loop {
        // Wait the poll interval to check again!
        sleep(Duration::from_secs(1)).await;

        // Shut down if there is a signal
        if let Ok(()) = rx.try_recv() {
            break;
        }

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if timestamp - last_updated < POLL_INTERVAL {
            continue;
        }

        // Retrieve the data from the elements
        let current_w = match element_value(&driver, By::Css("h2#pv-now b")).await {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Failed to retrieve current power: {}", error);
                continue;
            }
        };
        let total_kwh = match element_value(&driver, By::Css("h2#pv-to-date b")).await {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Failed to retrieve total energy production: {}", error);
                continue;
            }
        };
        last_updated = timestamp;

        // Update the status
        let mut status_guard = STATUS.lock().expect("Status mutex was poisoned");
        let status = Status {
            current_w,
            total_kwh,
            last_updated,
        };
        println!("⚡ Updated status to: {:#?}", status);
        status_guard.replace(status);
    }

    Ok(())
}

#[get("/", format = "application/json")]
async fn status() -> Option<Json<Status>> {
    let status_guard = STATUS.lock().expect("Status mutex was poisoined");
    status_guard.map(|status| Json(status))
}

#[rocket::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let mut driver_proc =
        spawn_driver(GECKO_DRIVER_PORT).expect("Could not find/start the Gecko Driver");

    let (tx, rx) = rocket::tokio::sync::oneshot::channel();
    let updater = rocket::tokio::spawn(update_loop(rx));

    let rocket = rocket::build().mount("/", routes![status]).ignite().await?;
    let shutdown = rocket.shutdown();

    select! {
        result = driver_proc.wait() => {
            shutdown.notify();
            tx.send(()).map_err(|_| eyre!("Could not send shutdown signal"))?;
            result?;
        },
        result = rocket.launch() => {
            tx.send(()).map_err(|_| eyre!("Could not send shutdown signal"))?;
            result?;
        },
        result = updater => {
            shutdown.notify();
            result??;
        }
    }

    Ok(())
}
