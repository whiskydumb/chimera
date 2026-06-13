# list recipes
default:
    @just --list

# format the codebase
fmt:
    cargo fmt --all

# format-check + clippy (what CI will enforce)
lint:
    cargo fmt --all --check
    cargo clippy --all-targets --all-features -- -D warnings

# run the test suite
test:
    cargo test --all

# everything CI runs
check: lint test

# run the TUI, or a subcommand: `just run`, `just run search docker`
run *args:
    cargo run -- {{args}}
