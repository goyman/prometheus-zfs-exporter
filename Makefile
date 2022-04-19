MDCAT:=$(firstword $(shell which mdcat cat 2>/dev/null))

.PHONY: help

help:
	@${MDCAT} MAKE.md

.PHONY: test

test: build
	-ssh vader pkill -f prometheus-zfs-exporter
	scp target/debug/prometheus-zfs-exporter vader:
	scp config-sample.toml vader:
	ssh vader ./prometheus-zfs-exporter -c config-sample.toml


.PHONY: build

build:
	cargo build

.PHONY: release

release:
	cargo build --release

.PHONY: run

run:
	cargo run


.PHONY: clean

clean:
	rm -fr target


