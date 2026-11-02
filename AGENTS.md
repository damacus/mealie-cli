# AGENTS.md — mealie-cli

`mealie-cli` is a Rust 2024 command-line client (binary: `mealie`) for the
[Mealie](https://mealie.io/) REST API. It is built for two audiences: humans at
a terminal, and scripts/agents consuming stable JSON.

## Using the CLI

Set `MEALIE_URL` and `MEALIE_TOKEN` in the environment first. HTTPS is
required by default; `USE_INSECURE_HTTP=yes` opts out for local testing. Check
setup with `mealie status` — it validates config, connectivity, and auth
without printing the token.

Commands (full reference: `README.md`, `docs/commands.md`):

```text
mealie status
mealie recipes search "<query>" [--limit N]
mealie recipes get <slug-or-exact-name>
mealie plan list [--from <date> --to <date>] [--type <meal-type>]
mealie plan week [--date <anchor> | --offset <N>]
mealie plan set --date <date> --type <meal-type> (--title <t> | --recipe <r>)
mealie plan delete --id <n>
```

Dates accept `YYYY-MM-DD`, `today`, `tomorrow`, `yesterday`, or signed offsets
(`+2d`, `-1w`). Meal types: `breakfast lunch dinner side snack drink dessert`.

For programmatic use, always pass `--json` (one document) or `--ndjson` (one
record per line); `--quiet` prints only IDs on successful mutations. Errors go
to stderr as JSON with stable codes: `missing_config`, `invalid_args`,
`not_found`, `ambiguous`, `authentication`, `api_error`, `network_error`.

`plan set` is conservative by design: exactly one of `--title`/`--recipe`;
recipe references resolve by exact slug, then exactly one case-insensitive
name (multiple matches → `ambiguous` with candidates); an existing entry is
replaced only for the same date+type, by creating the new entry first and then
deleting the original. If that delete fails, the new entry is rolled back
(deleted) and the original delete error is returned. More than one existing
match for the date+type aborts with `api_error` before anything is changed.

## Developing

### Layout

- `src/cli.rs` — clap command definitions
- `src/client.rs` — all HTTP calls (only file that talks to Mealie)
- `src/output.rs` — human/JSON/NDJSON rendering
- `src/error.rs` — `AppError` and the stable `ErrorCode` enum
- `src/config.rs`, `src/date_input.rs`, `src/meal_type.rs` — env config, date
  parsing, meal-type validation
- `tests/http.rs` — `mockito`-based API contract tests
- `docs/` — published docs site source; keep it in sync with behaviour

### Rules

- TDD: every behaviour-changing production line starts from a failing test.
  Mock HTTP with `mockito`; assert exact output strings where practical.
- Public contract — do not break silently: JSON record types and field names,
  the error-code enum, exit behaviour, `MEALIE_TOKEN` secrecy (never print,
  log, or commit it), and the `plan set` safety semantics. Contract changes
  need explicit approval and matching `README.md`/`docs/` updates.
- Small atomic changes; fish syntax for shell; no new dependencies without
  approval.

### Verify

```fish
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Looking up Mealie API details

The CLI only wraps the upstream API — endpoint paths, request/response fields,
pagination, and status codes are facts to look up, never guesses. Query the
grounded.tools documentation index through the `grounded` MCP server
(LiteLLM gateway `https://litellm.ironstone.casa/grounded_docs/mcp`; one-time
auth `devin mcp login grounded`, or an `x-litellm-api-key` /
`Authorization: Bearer` header from other clients).

Tools are prefixed `grounded_docs-`:

- `search_docs` — semantic search. Args: `library: "mealie"` (indexed at
  version `current`), `query: "<topic>"`, optional `version`/`limit`.
- `list_pages` — browse indexed page URLs; `read_page` — read one in full
  (chunked via `startChunk`).
- `find_version` — resolve the indexed version label.
- `fetch_url` — fetch any URL as Markdown, indexed or not.

Workflow: `search_docs` for the area (`"mealplans create"`, `"recipe payload
fields"`, `"API token auth"`) → `read_page` on the returned URL → implement
against that shape, and cite it in tests.

Endpoints in use today — verify against the docs before extending:

| Endpoint | Used for |
| --- | --- |
| `GET /api/users/self` | `mealie status` connectivity/auth check |
| `GET /api/recipes` | recipe search and exact-name resolution |
| `GET /api/recipes/{slug}` | `mealie recipes get` |
| `GET /api/households/mealplans` | `mealie plan list` / `plan week` |
| `POST /api/households/mealplans` | `mealie plan set` |
| `DELETE /api/households/mealplans/{id}` | `plan set` replace + `plan delete` |

Fallbacks without MCP: `$MEALIE_URL/openapi.json` (authoritative for the
deployed version), `$MEALIE_URL/docs`, <https://docs.mealie.io/>, or the CLI
`npx @arabold/docs-mcp-server@latest search mealie "<query>"`.

## Specialist personas

`.jules/sentinel.md` (security) and `.jules/bolt.md` (performance) contain
scoped personas with tighter constraints — read them when doing that work.
