.PHONY: dev run build install clean check test deps

# Watch for changes and rebuild automatically (requires cargo-watch)
dev:
	cargo watch -x run

# Run in debug mode
run:
	cargo run

# Build release binary
build:
	cargo build --release

# Install to /usr/local/bin
install: build
	sudo cp target/release/crabclip /usr/local/bin/crabclip

# Remove installed binary
uninstall:
	sudo rm -f /usr/local/bin/crabclip

# Run checks and tests
check:
	cargo check

test:
	cargo test

# Install system and cargo dependencies
deps:
	sudo apt install -y libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
	cargo install cargo-watch

# Remove build artifacts
clean:
	cargo clean
