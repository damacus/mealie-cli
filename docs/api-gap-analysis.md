# Mealie API gap analysis

This inventory compares the deployed Mealie v3.30.0 OpenAPI schema with the
catalogue CLI stack. API behaviour may differ on other Mealie versions. Coverage
below includes changes in the stack that are not in the 0.1.4 release.

## Recipes

| Capability | Mealie API | CLI coverage |
| --- | --- | --- |
| List, search and read | `GET /api/recipes`, `GET /api/recipes/{slug}` | `recipes list`, `search`, `get`; list can filter untagged and uncategorised recipes |
| Create, duplicate and delete | `POST /api/recipes`, `POST /api/recipes/{slug}/duplicate`, `DELETE /api/recipes/{slug}` | `recipes create`, `duplicate`, `delete` |
| Partial and full updates | `PATCH` and `PUT /api/recipes/{slug}`; `PATCH` and `PUT /api/recipes` | `recipes update`, `patch`, `replace`, `batch-update` |
| Suggestions and last made | `GET /api/recipes/suggestions`, `PATCH /api/recipes/{slug}/last-made` | `recipes suggest`, `last-made` |
| Import from URL or HTML/JSON | `POST /api/recipes/create/url`, `POST /api/recipes/create/html-or-json` | `recipes import-url`, `import-file` |
| Bulk URL import | `POST /api/recipes/create/url/bulk`, `GET /api/groups/reports/{id}` | `recipes bulk-import`, `import-report`; the report tracks asynchronous work |
| ZIP and AI-assisted import | `POST /api/recipes/create/zip`, `POST /api/recipes/create/ai` | `recipes import-zip`, `import-ai` |
| Export a recipe | `GET /api/recipes/exports`, `GET /api/recipes/{slug}/exports` | `recipes export-formats`, `export` |
| Recipe image | `GET /api/media/recipes/{id}/images/{name}`, `POST`, `PUT` and `DELETE /api/recipes/{slug}/image` | `recipes image get`, `set-url`, `upload`, `delete` |
| Bulk assignment and settings | `POST /api/recipes/bulk-actions/tag`, `/categorize`, `/settings` | `recipes bulk-tag`, `bulk-categorize`, `bulk-settings`; results are verified per recipe |
| Bulk delete | `POST /api/recipes/bulk-actions/delete` | `recipes bulk-delete`; requires confirmation and checks for partial results |
| Bulk export | `POST` and `GET /api/recipes/bulk-actions/export`, its download endpoint and `DELETE /purge` | `recipes bulk-export start`, `list`, `download`, `purge` |

Mealie no longer provides the legacy untagged or uncategorised summary
endpoints. The CLI filters the current recipe summary data instead. Recipe
comments, attachments, timeline events, shared recipes and the ingredient
parser are separate API groups and are not represented by these commands.

## Tags and categories

Both `/api/organizers/tags` and `/api/organizers/categories` expose the same
core operations. The CLI provides them under `tags` and `categories`:

| API operation | CLI coverage |
| --- | --- |
| List and list empty organisers | `list`, `list --empty` |
| Create | `create` |
| Retrieve by ID or slug | `get --id`, `get --slug` |
| Update name | `rename` |
| Merge and delete | `merge`, `delete`; confirmation or dry run required |
| Assign to or remove from a recipe | `recipes organize`; add to multiple recipes with `bulk-tag` or `bulk-categorize` |

## Other API families

The server also exposes meal plans, shopping lists, cookbooks, foods, units,
tools, sharing, household/group management and administration. The CLI already
supports meal-plan list, week, set and delete; the other families need their own
gap analysis before implementation.
