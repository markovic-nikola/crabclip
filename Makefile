.PHONY: help dev run build build-release test fmt clippy lint deb install uninstall clean release deps

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

dev: ## Live reload with cargo-watch
	@command -v cargo-watch >/dev/null 2>&1 || { echo "Install cargo-watch first: cargo install cargo-watch"; exit 1; }
	cargo watch -x run

run: ## Run debug build
	cargo run

build: ## Debug build
	cargo build

build-release: ## Release build
	cargo build --release

test: ## Run all tests
	cargo test --all

fmt: ## Format code
	cargo fmt --all

clippy: ## Run clippy lints
	cargo clippy --all-targets -- -W clippy::all

lint: ## Run all checks (fmt, clippy, tests)
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -W clippy::all
	cargo test --all

deb: build-release ## Build .deb package
	cargo deb --no-build

install: build-release ## Install to /usr/local/bin
	sudo cp target/release/crabclip /usr/local/bin/crabclip

uninstall: ## Uninstall from /usr/local/bin
	sudo rm -f /usr/local/bin/crabclip

deps: ## Install system and cargo dependencies
	sudo apt install -y libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
	cargo install cargo-watch cargo-deb

release: ## Bump patch version, tag, and push
	./scripts/release.sh

clean: ## Clean build artifacts
	cargo clean
