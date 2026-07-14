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

Check configuration, connectivity, and authentication without exposing the token:

```text
mealie status
```

The command exits with the existing stable error code for the first failed check. Use
`--json` or `--ndjson` to receive the same status as a stable machine-readable record.

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

Check whether the CLI is ready to use:

```text
mealie status
```

Search recipes:

```text
mealie recipes search "pesto chicken" --limit 5
```

Get a recipe and its complete ingredient list by slug or exact name:

```text
mealie recipes get butter-chicken
```

List meal plans:

```text
mealie plan list
```

With no date flags, this lists from today through Sunday using the local timezone configured on the computer running the command. Date flags accept `YYYY-MM-DD`, `today`, `tomorrow`, `yesterday`, and signed offsets such as `+2d`, `-1d`, `+1w`, or `-1w`:

```text
mealie plan list --from today --to +3d
mealie plan set --date tomorrow --type dinner --title "Bolognaise"
```

Filter meal plans by type:

```text
mealie plan list --from 2026-05-13 --to 2026-05-16 --type dinner
```

View a complete Monday-to-Sunday week, including days with no meals. With no
options it shows the current local week; use an anchor date or signed whole-week
offset to choose another week:

```text
mealie plan week
mealie plan week --date 2026-05-13
mealie plan week --offset -1
```

The human view is grouped by day and switches to a stacked ASCII-only form when
`COLUMNS` is below 60. `--json` and `--ndjson` keep emitting the existing stable
`plan_entry` records (or the existing empty record), so terminal layout never
changes automation output.

Create or replace a plain-text meal plan entry:

```text
mealie plan set --date 2026-05-13 --type dinner --title "Bolognaise"
```

Create or replace a meal plan entry from a recipe slug or exact name:

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
- Recipe references first match an exact slug. If the slug does not exist, they may match one exact case-insensitive recipe name. Multiple exact name matches return an `ambiguous` error with candidate names and slugs; fuzzy search results are never chosen.
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
