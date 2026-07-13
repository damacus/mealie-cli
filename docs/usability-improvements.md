# Usability improvements

This pass compared every current `mealie` command and its error paths with the output conventions used by `freeagent-cli`. It delivered these 20 changes:

1. Human-readable output is now the default.
2. Recipe searches render as aligned tables.
3. Empty recipe searches name the query that returned no results.
4. Recipe detail output uses labelled fields.
5. Meal-plan lists render as aligned tables.
6. Empty meal-plan lists show the requested date range.
7. New meal-plan entries get a concise creation confirmation.
8. Replaced meal-plan entries get a distinct replacement confirmation.
9. Deleted meal-plan entries identify the removed ID.
10. Errors are written to stderr, leaving stdout safe for pipelines.
11. Interactive errors use plain-language `Error` and `Hint` messages.
12. `--json` and `--ndjson` keep errors machine-readable.
13. Missing configuration errors explain how to recover.
14. API errors include useful error details returned by Mealie.
15. Authentication failures have their own stable error code and hint.
16. Error categories have distinct, documented process exit codes.
17. Invalid dates identify the flag and rejected value.
18. Reversed meal-plan date ranges fail before making an API request.
19. Meal types are validated by the argument parser, which lists allowed values in help and errors.
20. Recipe search limits are validated against the supported 1-100 range.

Automation remains supported through explicit `--json`, `--ndjson`, and `--quiet` output modes.
