use std::thread;
use std::time::{Duration, SystemTime};
use webdriver_client::firefox::GeckoDriver;
use webdriver_client::messages::{LocationStrategy, NewSessionCmd};
use webdriver_client::{DriverSession, Error};

const USERNAME: &'static str = "pja@vtilburg.net";
const PASSWORD: &'static str = "XXXXXXXXXXXXXXXX";
const URL: &'static str = "https://my.autarco.com/";

fn main() -> Result<(), Error> {
    let driver = Box::new(GeckoDriver::spawn()?);
    let session = DriverSession::create_session(driver, &NewSessionCmd::default())?;

    session.go(URL)?;

    // Log in
    let input = session.find_element("input#username", LocationStrategy::Css)?;
    input.send_keys(USERNAME)?;
    let input = session.find_element("input#password", LocationStrategy::Css)?;
    input.send_keys(PASSWORD)?;
    let input = session.find_element("button[type=submit]", LocationStrategy::Css)?;
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

        let current = session.find_element("h2#pv-now b", LocationStrategy::Css)?;
        println!("current: {} W", current.text()?);

        let total = session.find_element("h2#pv-to-date b", LocationStrategy::Css)?;
        println!("total: {} kWh", total.text()?);

        println!();
    }
}
