# Installation

## From Source

```bash
git clone https://github.com/damacus/mealie-cli.git
cd mealie-cli
cargo build --release
```

The binary is written to:

```text
target/release/mealie
```

## From crates.io

After the crate is published:

```bash
cargo install mealie-cli
```

## Requirements

- Rust stable
- A reachable Mealie instance
- A Mealie API token

## Development Checks

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo package --allow-dirty
```
