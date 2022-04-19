# Prometheus ZFS exporter

This exporter is for mainly for monitoring ZFS snapshot age.

It will also report zpool status, and general metrics.

It is written in **Rust** and uses **TOML** as configuration language.

Documentation is mostly located within the sample config file.

## Usage

`./prometheus-zfs-exporter -c config.toml`

The config file defaults to `/etc/prometheus/zfs-exporter.toml`

## Reload

Config and pool/dataset list is loaded only when this is launched. If you add
pool or datasets, you need to restart the exporter.

## Use with ZFS autobackup

I made this exporter to monitor the age of snapshots created by [ZFS
autobackup](https://github.com/psy0rz/zfs_autobackup) which is a handy python
tool that will create snapshot and replicate them.

The provided `config-sample.toml` is tailored to be used with ZFS autobackup
configuration.


## Alternatives

Here are other exporters that can be used as alternatives:

- https://github.com/pdf/zfs_exporter
- https://github.com/eliothedeman/zfs_exporter
- https://github.com/ncabatoff/zfs-exporter
- https://github.com/eripa/prometheus-zfs
