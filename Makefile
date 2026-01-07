.PHONY: help build release test clean install run bench

help:
	@echo "Available targets:"
	@echo "  build    - Build debug binary"
	@echo "  release  - Build optimized release binary"
	@echo "  test     - Run all tests"
	@echo "  clean    - Clean build artifacts"
	@echo "  install  - Install binary to ~/.cargo/bin"
	@echo "  run      - Run debug binary (usage: make run ARGS='archive.zip')"
	@echo "  bench    - Run benchmarks"

build:
	cargo build

release:
	cargo build --release

test:
	cargo test

clean:
	cargo clean

install:
	cargo install --path .

run:
	cargo run -- $(ARGS)

bench:
	cargo test --release -- --nocapture
