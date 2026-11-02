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
mealie plan list
```

Without a date flag, `plan list` uses today through Sunday in the local timezone of the computer running the command. With only `--from`, it uses that date through the Sunday of the same ISO week; with only `--to`, it uses the Monday of that ISO week through that date. The resolved start date must be on or before the resolved end date. `meal-plan` is an alias for `plan`.

Dates accept ISO `YYYY-MM-DD`, `today`, `tomorrow`, `yesterday`, and signed offsets: `+Nd`, `-Nd`, `+Nw`, or `-Nw`. For example:

```bash
mealie plan list --from today --to +3d
mealie plan set --date tomorrow --type dinner --title "Bolognaise"
```

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

# Catalogue commands

`mealie recipes list` lists the complete paginated catalogue. Add `--untagged`
or `--uncategorised` to filter recipes with empty tags or categories. Both
filters may be combined. `mealie tags list` and `mealie categories list` list
existing organisers. Use `--json` or `--ndjson` for structured results.
Use `mealie tags create --name <name>` or `mealie categories create --name
<name>` to create one; add `--dry-run` to preview it.

Use `mealie tags get --slug <slug>` or `--id <id>` to inspect an organiser;
the same commands work under `categories`. Rename one with `rename --id <id>
--name <name>`. Merge with `merge --from <id> --to <id>`, or delete with
`delete --id <id>`. Merge removes the source organiser and delete removes the
specified organiser: both require either `--yes` to execute or `--dry-run` to
preview the action. Rename also supports `--dry-run`.

`mealie recipes update <slug> --description <text>` updates a description.
Add `--dry-run` to show the proposed change without sending it to Mealie.
Structured `recipes get` output includes catalogue metadata when supplied by
the server.

Use `mealie recipes organize <slug> --add-tag <id>` or `--remove-tag <id>`
to change tags. Use `--add-category <id>` or `--remove-category <id>` for
categories. Supply one change per invocation; `--dry-run` reads the current
recipe and previews the resulting assignments without changing Mealie. The
command reads and then replaces the selected assignment list, so avoid editing
the same list concurrently in another client.
