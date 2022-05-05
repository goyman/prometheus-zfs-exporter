use crate::config::Config;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::BufRead;
use std::io::BufReader;
use std::process::Command;
use std::process::Stdio;

use prometheus_exporter::prometheus::register_int_gauge_vec;
use prometheus_exporter::prometheus::IntGaugeVec;

struct Snapshot {
    name: String,
    match_regex: Regex,
}

struct Dataset {
    name: String,
    snapshots: Vec<String>,
}

struct Pool {
    name: String,
}

pub struct Scraper {
    datasets: HashMap<String, Dataset>,
    pools: HashMap<String, Pool>,
    snapshots: HashMap<String, Snapshot>,
    dataset_snapshot_gauge: IntGaugeVec,
    dataset_used_gauge: IntGaugeVec,
    dataset_available_gauge: IntGaugeVec,
    pool_health_gauge: IntGaugeVec,
    pool_allocated_gauge: IntGaugeVec,
    pool_free_gauge: IntGaugeVec,
    pool_size_gauge: IntGaugeVec,
}

impl Scraper {
    pub fn from_config(config: Config) -> Result<Self> {
        let mut pools = HashMap::new();
        let mut datasets = HashMap::new();
        let mut snapshots = HashMap::new();

        for snapshot in config.snapshots {
            let re = Regex::new(&snapshot.match_string)
                .with_context(|| "Cannot parse snapshot match regex")?;
            snapshots.insert(
                snapshot.name.clone(),
                Snapshot {
                    match_regex: re,
                    name: snapshot.name,
                },
            );
        }

        if config.all_pools {
            for line in
                Self::run_command("zpool", ["list", "-H", "-o", "name"])?
            {
                pools.insert(line.clone(), Pool { name: line });
            }
        }

        for pool in config.pools {
            // Just override the pool, there is no config
            pools.insert(pool.name.clone(), Pool { name: pool.name });
        }

        if config.all_datasets {
            for line in Self::run_command(
                "zfs",
                ["list", "-H", "-o", "name", "-t", "filesystem"],
            )? {
                datasets.insert(
                    line.clone(),
                    Dataset {
                        name: line,
                        snapshots: vec![],
                    },
                );
            }
        }

        for dataset in config.datasets {
            let snapshot_keys = dataset.snapshots;
            for name in &snapshot_keys {
                if !snapshots.contains_key(name) {
                    bail!("Snapshot {} not defined", name);
                }
            }
            if dataset.recurse {
                for line in Self::run_command(
                    "zfs",
                    [
                        "list",
                        "-H",
                        "-o",
                        "name",
                        "-t",
                        "filesystem",
                        "-r",
                        &dataset.name,
                    ],
                )? {
                    datasets
                        .entry(line.clone())
                        .and_modify(|e| e.snapshots = snapshot_keys.clone())
                        .or_insert_with(|| Dataset {
                            name: line,
                            snapshots: snapshot_keys.clone(),
                        });
                }
            }
            datasets
                .entry(dataset.name.clone())
                .and_modify(|e| e.snapshots = snapshot_keys.clone())
                .or_insert_with(|| Dataset {
                    name: dataset.name,
                    snapshots: snapshot_keys,
                });
        }

        let dataset_snapshot_gauge = register_int_gauge_vec!(
            "zfs_dataset_latest_snapshot_timestamp_seconds",
            "time in seconds since EPOCH of latest snapshot matching expression",
            &["dataset", "name"]
        )?;

        let dataset_used_gauge = register_int_gauge_vec!(
            "zfs_dataset_used_bytes",
            "used bytes on dataset",
            &["dataset"]
        )?;

        let dataset_available_gauge = register_int_gauge_vec!(
            "zfs_dataset_available_bytes",
            "available bytes on dataset",
            &["dataset"]
        )?;

        let pool_health_gauge = register_int_gauge_vec!(
            "zfs_pool_health",
            "zfs pool status, [0: ONLINE, 1: DEGRADED, 2: FAULTED, 3: OFFLINE, 4: UNAVAIL, 5: REMOVED, 6: SUSPENDED]",
            &["pool"]
        )?;

        let pool_allocated_gauge = register_int_gauge_vec!(
            "zfs_pool_allocated_bytes",
            "zfs allocated bytes",
            &["pool"]
        )?;

        let pool_free_gauge = register_int_gauge_vec!(
            "zfs_pool_free_bytes",
            "zfs free bytes",
            &["pool"]
        )?;

        let pool_size_gauge = register_int_gauge_vec!(
            "zfs_pool_size_bytes",
            "zfs size bytes",
            &["pool"]
        )?;

        let scraper = Scraper {
            datasets,
            pools,
            snapshots,
            dataset_snapshot_gauge,
            dataset_used_gauge,
            dataset_available_gauge,
            pool_health_gauge,
            pool_allocated_gauge,
            pool_free_gauge,
            pool_size_gauge,
        };

        Ok(scraper)
    }

    pub fn update(&self) -> Result<()> {
        for (dataset_name, ds) in &self.datasets {
            let (used, available) = Self::read_dataset_stats(&dataset_name)?;

            self.dataset_used_gauge
                .get_metric_with_label_values(&[&dataset_name])?
                .set(used);
            self.dataset_available_gauge
                .get_metric_with_label_values(&[&dataset_name])?
                .set(available);

            for ref_name in &ds.snapshots {
                let snapshot = self
                    .snapshots
                    .get(ref_name)
                    .ok_or(anyhow!("Snapshot config missing"))?;

                let mut latest = HashMap::new();

                for line in Self::run_command(
                    "zfs",
                    [
                        "list",
                        "-H",
                        "-o",
                        "name,creation",
                        "-p",
                        "-t",
                        "snapshot",
                        dataset_name,
                    ],
                )? {
                    let fields: Vec<_> = line.split("\t").collect();

                    let full_name =
                        fields.get(0).ok_or(anyhow!("Cannot parse line"))?;
                    let date: i64 = fields
                        .get(1)
                        .ok_or(anyhow!("Cannot parse line"))?
                        .parse()?;

                    let snapshot_name = full_name
                        .split("@")
                        .collect::<Vec<_>>()
                        .get(1)
                        .ok_or(anyhow!("Cannot snapshot name"))?
                        .clone();

                    let captures = snapshot.match_regex.captures(snapshot_name);

                    if let Some(captures) = captures {
                        let _id = captures
                            .name("id")
                            .ok_or(anyhow!(
                                "No id capture in snapshot match regex"
                            ))?
                            .as_str();
                        let label = captures
                            .name("label")
                            .ok_or(anyhow!(
                                "No label capture in snapshot match regex"
                            ))?
                            .as_str()
                            .to_owned();

                        latest
                            .entry(label)
                            .and_modify(|e| {
                                if date > *e {
                                    *e = date;
                                }
                            })
                            .or_insert(date);
                    }
                }

                for (name, date) in latest {
                    self.dataset_snapshot_gauge
                        .get_metric_with_label_values(&[&dataset_name, &name])?
                        .set(date);
                }
            }
        }

        for (pool_name, _pool) in &self.pools {
            for line in Self::run_command(
                "zpool",
                [
                    "list",
                    "-p",
                    "-H",
                    "-o",
                    "size,alloc,free,health",
                    &pool_name,
                ],
            )? {
                let fields: Vec<_> = line.split("\t").collect();

                let size: i64 = fields
                    .get(0)
                    .ok_or(anyhow!("Cannot parse line"))?
                    .parse()?;
                let alloc: i64 = fields
                    .get(1)
                    .ok_or(anyhow!("Cannot parse line"))?
                    .parse()?;
                let free: i64 = fields
                    .get(2)
                    .ok_or(anyhow!("Cannot parse line"))?
                    .parse()?;
                let health =
                    fields.get(3).ok_or(anyhow!("Cannot parse line"))?;

                let health = match *health {
                    "ONLINE" => 0,
                    "DEGRADED" => 1,
                    "FAULTED" => 2,
                    "OFFLINE" => 3,
                    "UNAVAIL" => 4,
                    "REMOVED" => 5,
                    "SUSPENDED" => 6,
                    _ => -1,
                };

                self.pool_size_gauge
                    .get_metric_with_label_values(&[&pool_name])?
                    .set(size);

                self.pool_allocated_gauge
                    .get_metric_with_label_values(&[&pool_name])?
                    .set(alloc);

                self.pool_free_gauge
                    .get_metric_with_label_values(&[&pool_name])?
                    .set(free);

                self.pool_health_gauge
                    .get_metric_with_label_values(&[&pool_name])?
                    .set(health);
            }
        }

        Ok(())
    }

    fn read_dataset_stats(name: &str) -> Result<(i64, i64)> {
        let mut used: i64 = 0;
        let mut available: i64 = 0;

        for line in Self::run_command(
            "zfs",
            ["get", "-H", "used,available", "-p", name],
        )? {
            let fields: Vec<_> = line.split("\t").collect();

            let prop = fields.get(1).ok_or(anyhow!("Cannot parse line"))?;
            let val: i64 =
                fields.get(2).ok_or(anyhow!("Cannot parse line"))?.parse()?;

            match *prop {
                "used" => used = val,
                "available" => available = val,
                _ => {}
            }
        }

        Ok((used, available))
    }

    fn run_command<I, S>(cmd: S, args: I) -> Result<Vec<String>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let child = Command::new(cmd)
            .args(args)
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| "Cannot execute command")?;

        let out = child
            .wait_with_output()
            .with_context(|| "Cannot capture stdout")?;

        let stdout = String::from_utf8_lossy(&out.stdout);

        stdout
            .lines()
            .map(|line| Ok(line.trim().to_string()))
            .collect()
    }
}
