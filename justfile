_default:
    @just --list

# Build in debug mode
build:
    cargo build

# Run formatting, linting, and tests
check:
    cargo fmt --all --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test

# Build in release mode
release:
    cargo build --release

# Run the app
run *ARGS:
    cargo run -- {{ARGS}}

# Install to ~/.local/bin
install: release
    cp target/release/ghx ~/.local/bin/
    codesign -s - ~/.local/bin/ghx

# Uninstall from ~/.local/bin
uninstall:
    rm -f ~/.local/bin/ghx

# Remove build artifacts
clean:
    cargo clean
