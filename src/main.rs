// Will create an exporter with a single metric that will randomize the value
// of the metric everytime the exporter duration times out.


mod config;
mod options;
mod scraper;

use anyhow::Result;
use config::Config;
use options::Options;
use scraper::Scraper;
use structopt::StructOpt;

#[async_std::main]
async fn main() -> Result<()> {
    let options = Options::from_args();
    let config = Config::read(&options.config)?;

    let addr = config.listen;
    let interval = config.interval;

    let scraper = Scraper::from_config(config)?;

    let exporter = prometheus_exporter::start(addr)?;

    let duration = std::time::Duration::from_millis(interval);

    loop {
        scraper.update()?;
        let _guard = exporter.wait_duration(duration);
    }

    Ok(())
}
