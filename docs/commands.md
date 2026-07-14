# Commands

## Status

Check whether the URL and token are configured, then verify connectivity and authentication:

```bash
mealie status
```

The token is never printed. Checks that cannot run are reported as `not checked`.
Use `--json` or `--ndjson` for a stable status record suitable for automation.

## Recipes

Search recipes:

```bash
mealie recipes search "pesto chicken" --limit 5
```

`--limit` accepts values from 1 to 100. `recipe` is a singular alias for `recipes`.

Get one recipe and its complete ingredient list by slug or exact name:

```bash
mealie recipes get butter-chicken
```

## Meal Plans

List entries:

```bash
mealie plan list --from 2026-05-13 --to 2026-05-16
```

The start date must be on or before the end date. `meal-plan` is an alias for `plan`.

Filter by meal type:

```bash
mealie plan list --from 2026-05-13 --to 2026-05-16 --type dinner
```

Create or replace a plain-text meal:

```bash
mealie plan set --date 2026-05-13 --type dinner --title "Bolognaise"
```

Create or replace a recipe-backed meal:

```bash
mealie plan set --date 2026-05-16 --type dinner --recipe pesto-chicken-stew-with-cheesy-dumplings
```

Delete an entry:

```bash
mealie plan delete --id 123
```

Valid meal types:

```text
breakfast lunch dinner side snack drink dessert
```

## Mutation Safety

`plan set` requires exactly one of `--title` or `--recipe`.

Recipe references first match an exact slug. If that slug does not exist, the CLI accepts one exact case-insensitive recipe name. Several exact name matches return an `ambiguous` error listing candidate names and slugs. The CLI never guesses from fuzzy search results.
