use color_eyre::Result;
use std::thread;
use std::time::{Duration, SystemTime};
use thirtyfour_sync::prelude::*;

const USERNAME: &'static str = "pja@vtilburg.net";
const PASSWORD: &'static str = "XXXXXXXXXXXXXXXX";
const URL: &'static str = "https://my.autarco.com/";

const GECKO_DRIVER_PORT: u16 = 18019;

use std::process::{Child, Command, Stdio};

struct GeckoDriver(Child);

impl GeckoDriver {
    pub fn spawn() -> Result<Self> {
        // This is taken from the webdriver-client crate.
        let child = Command::new("geckodriver")
            .arg("-b")
            .arg("firefox")
            .arg("--port")
            .arg(format!("{}", GECKO_DRIVER_PORT))
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

#[derive(Debug)]
struct Status {
    current_w: u32,
    total_kwh: u32,
    last_updated: u64,
}

fn login(driver: &WebDriver) -> Result<()> {
    driver.get(URL)?;

    let input = driver.find_element(By::Id("username"))?;
    input.send_keys(USERNAME)?;
    let input = driver.find_element(By::Id("password"))?;
    input.send_keys(PASSWORD)?;
    let input = driver.find_element(By::Css("button[type=submit]"))?;
    input.click()?;

    Ok(())
}

fn element_value(driver: &WebDriver, by: By) -> Result<u32> {
    let element = driver.find_element(by)?;
    let text = element.text()?;
    let value = text.parse()?;

    Ok(value)
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let _gecko_driver = GeckoDriver::spawn()?;
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless()?;
    let driver = WebDriver::new(&format!("http://localhost:{}", GECKO_DRIVER_PORT), &caps)?;

    // Go to the My Autarco site and login
    login(&driver)?;

    loop {
        // // Take a screenshot!
        // driver.screenshot(&std::path::PathBuf::from("screenshot.png"))?;

        // Retrieve the data from the elements
        let last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let current_w = match element_value(&driver, By::Css("h2#pv-now b")) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Failed to retrieve current power: {}", error);
                continue;
            }
        };
        let total_kwh = match element_value(&driver, By::Css("h2#pv-to-date b")) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Failed to retrieve total energy production: {}", error);
                continue;
            }
        };

        let status = Status {
            current_w,
            total_kwh,
            last_updated,
        };
        dbg!(&status);

        thread::sleep(Duration::from_secs(60));
    }
}
