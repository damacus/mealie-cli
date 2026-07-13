# mealie-cli

`mealie-cli` is a small Rust command-line client for the [Mealie](https://www.mealie.io/) REST API.
It is designed for both interactive use and automation: commands are readable by default, with stable JSON formats available when a script needs them.

## Install

Build the `mealie` binary from this repository:

```text
cargo build --release
```

The executable is written to:

```text
target/release/mealie
```

## Configuration

Set both environment variables before running commands:

```text
MEALIE_URL=https://mealie.example.com
MEALIE_TOKEN=<token>
```

`MEALIE_URL` may include a trailing slash. `MEALIE_TOKEN` is sent as a bearer token and is never printed by the CLI.

HTTPS is required by default. For a deliberately insecure local or isolated
network, explicitly opt in to HTTP:

```text
USE_INSECURE_HTTP=yes
```

This override allows the bearer token to travel without transport encryption
and should not be used on untrusted networks.

## Output

Human-readable output is the default. Lists use aligned tables, detail commands use labelled fields, and mutations say what changed:

```text
NAME           SLUG           ID
Pesto Chicken  pesto-chicken  2f34...
```

Use `--json` for one pretty JSON array or `--ndjson` for one JSON object per line:

```json
{"ok":true,"type":"recipe","id":"uuid","slug":"recipe-slug","name":"Recipe Name"}
```

Global output flags:

```text
--json     print one pretty JSON document
--ndjson   print NDJSON
--quiet    quiet mode; print only IDs for successful changes
```

Errors are written to stderr and include a practical hint when one is available:

```text
Error: MEALIE_URL is required
Hint: Set MEALIE_URL and MEALIE_TOKEN, then run the command again.
```

With `--json` or `--ndjson`, errors remain machine-readable on stderr:

```json
{"ok":false,"error":"not_found","message":"get recipe returned 404"}
```

Stable error codes:

```text
missing_config
invalid_args
not_found
ambiguous
authentication
api_error
network_error
```

## Commands

Search recipes:

```text
mealie recipes search "pesto chicken" --limit 5
```

Get a recipe and its complete ingredient list by exact slug:

```text
mealie recipes get butter-chicken
```

List meal plans:

```text
mealie plan list --from 2026-05-13 --to 2026-05-16
```

Filter meal plans by type:

```text
mealie plan list --from 2026-05-13 --to 2026-05-16 --type dinner
```

Create or replace a plain-text meal plan entry:

```text
mealie plan set --date 2026-05-13 --type dinner --title "Bolognaise"
```

Create or replace a meal plan entry from a recipe slug:

```text
mealie plan set --date 2026-05-16 --type dinner --recipe pesto-chicken-stew-with-cheesy-dumplings
```

Delete a meal plan entry:

```text
mealie plan delete --id 123
```

Valid meal types:

```text
breakfast lunch dinner side snack drink dessert
```

## Safety

`plan set` is intentionally conservative:

- It requires exactly one of `--title` or `--recipe`.
- `--recipe` must be an exact recipe slug.
- Existing entries are replaced only for the same `date + type`.
- The CLI deletes and recreates entries instead of updating in place, avoiding accidental preservation or mutation of unknown API fields.

## Development

Run the test suite:

```text
cargo test
```

Check formatting:

```text
cargo fmt --check
```

Run clippy as CI does:

```text
cargo clippy --all-targets --all-features -- -D warnings
```
