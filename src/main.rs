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

fn main() -> Result<()> {
    color_eyre::install()?;

    let _gecko_driver = GeckoDriver::spawn()?;
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless()?;
    let driver = WebDriver::new(&format!("http://localhost:{}", GECKO_DRIVER_PORT), &caps)?;

    // Got to the My Autarco site
    driver.get(URL)?;

    // Log in
    let input = driver.find_element(By::Id("username"))?;
    input.send_keys(USERNAME)?;
    let input = driver.find_element(By::Id("password"))?;
    input.send_keys(PASSWORD)?;
    let input = driver.find_element(By::Css("button[type=submit]"))?;
    input.click()?;

    loop {
        thread::sleep(Duration::from_secs(60));

        // let screenshot = session.screenshot()?;
        // screenshot.save_file("screenshot.png")?;

        // Retrieve the data from the elements
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        println!("time: {}", time);

        let current = driver.find_element(By::Css("h2#pv-now b"))?;
        println!("current: {} W", current.text()?);

        let total = driver.find_element(By::Css("h2#pv-to-date b"))?;
        println!("total: {} kWh", total.text()?);

        println!();
    }
}
