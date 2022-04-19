use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Options {
    #[structopt(
        parse(from_os_str),
        short = "c",
        long = "config",
        default_value = "/etc/prometheus/zfs-exporter.toml"
    )]
    pub config: PathBuf,
}
