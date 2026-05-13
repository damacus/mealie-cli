# Output

NDJSON is the default output format.

```json
{"ok":true,"type":"recipe","id":"uuid","slug":"recipe-slug","name":"Recipe Name"}
```

## Global Flags

```text
--json     print one pretty JSON document
--ndjson   print NDJSON, default
--quiet    print only mutation ids, or nothing for successful reads
```

## Errors

Errors are machine-readable:

```json
{"ok":false,"error":"not_found","message":"get recipe returned 404"}
```

Stable error codes:

```text
missing_config
invalid_args
not_found
ambiguous
api_error
network_error
```
