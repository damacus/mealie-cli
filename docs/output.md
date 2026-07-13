# Output

Human-readable output is the default. Lists are aligned tables, individual resources use labelled fields, empty results explain what was searched, and mutations confirm what changed.

```text
NAME           SLUG           ID
Pesto Chicken  pesto-chicken  uuid
```

NDJSON remains available explicitly with `--ndjson`:

```json
{"ok":true,"type":"recipe","id":"uuid","slug":"recipe-slug","name":"Recipe Name"}
```

## Global Flags

```text
--json     print one pretty JSON document
--ndjson   print NDJSON
--quiet    quiet mode; print only IDs for successful changes
```

## Errors

Errors go to stderr. In the default mode they use plain language and may include a recovery hint. With `--json` or `--ndjson`, they use a machine-readable envelope:

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

Exit codes are stable: `1` for API/ambiguous failures, `2` for arguments or configuration, `3` for not found, `4` for authentication, and `5` for network failures.
