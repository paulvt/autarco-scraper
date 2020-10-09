use color_eyre::Result;
use lazy_static::lazy_static;
use rocket::{get, launch, routes, Rocket};
use rocket_contrib::json::Json;
use serde::Serialize;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime};
use thirtyfour::prelude::*;

const URL: &'static str = "https://my.autarco.com/";
const USERNAME: &'static str = "pja@vtilburg.net";
const PASSWORD: &'static str = "XXXXXXXXXXXXXXXX";
const POLL_INTERVAL: u64 = 300;

const GECKO_DRIVER_PORT: u16 = 18019;

struct GeckoDriver(Child);

impl GeckoDriver {
    pub fn spawn(port: u16) -> Result<Self> {
        // This is taken from the webdriver-client crate.
        let child = Command::new("geckodriver")
            .arg("-b")
            .arg("firefox")
            .arg("--port")
            .arg(format!("{}", port))
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()?;
        thread::sleep(Duration::new(1, 500));

        Ok(GeckoDriver(child))
    }
}

impl Drop for GeckoDriver {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Status {
    current_w: u32,
    total_kwh: u32,
    last_updated: u64,
}

async fn login(driver: &WebDriver) -> Result<()> {
    driver.get(URL).await?;

    let input = driver.find_element(By::Id("username")).await?;
    input.send_keys(USERNAME).await?;
    let input = driver.find_element(By::Id("password")).await?;
    input.send_keys(PASSWORD).await?;
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

async fn update_loop() -> Result<()> {
    color_eyre::install()?;

    let _gecko_driver = GeckoDriver::spawn(GECKO_DRIVER_PORT)?;
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless()?;
    let driver = WebDriver::new(&format!("http://localhost:{}", GECKO_DRIVER_PORT), &caps).await?;

    // Go to the My Autarco site and login
    login(&driver).await?;

    loop {
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
        let last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Update the status
        let mut status_guard = STATUS.lock().expect("Status mutex was poisoned");
        let status = Status {
            current_w,
            total_kwh,
            last_updated,
        };
        println!("Updated status to: {:#?}", status);
        status_guard.replace(status);
        drop(status_guard);

        // Wait the poll interval to check again!
        thread::sleep(Duration::from_secs(POLL_INTERVAL));
    }
}

#[get("/", format = "application/json")]
fn status() -> Option<Json<Status>> {
    let status_guard = STATUS.lock().expect("Status mutex was poisoned");
    status_guard.map(|status| Json(status))
}

#[launch]
fn rocket() -> Rocket {
    rocket::tokio::spawn(async { update_loop().await });

    rocket::ignite().mount("/", routes![status])
}
