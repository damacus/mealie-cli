# Mealie CLI

`mealie-cli` is a Rust command-line client for the Mealie REST API.

It is designed for automation and LLM-facing workflows:

- compact NDJSON by default
- stable JSON keys
- machine-readable errors
- conservative mutation behavior

## Quick Start

```bash
git clone https://github.com/damacus/mealie-cli.git
cd mealie-cli
cargo build --release
```

Configure access:

```bash
export MEALIE_URL=https://mealie.example.com
export MEALIE_TOKEN=your-token
```

Run a read-only command:

```bash
./target/release/mealie recipes search "pesto chicken" --limit 5
```

## Guides

- [Installation](installation.md)
- [Configuration](configuration.md)
- [Commands](commands.md)
- [Output](output.md)
- [Docker](docker.md)
