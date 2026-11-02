# Mealie API gap analysis

Compared with the deployed Mealie v3.30.0 OpenAPI schema and the CLI's main branch.
The server's schema is authoritative for its version; other versions can differ.

| Capability | Mealie API | CLI on main |
| --- | --- | --- |
| Recipes | List, retrieve, create, update, delete, import, export and bulk actions | Search and limited recipe details |
| Categories and tags | List, create, retrieve, update, merge and delete | Not available |
| Images | Set from URL, upload or delete | Not available |
| Meal plans | List and manage entries and rules | List, week, set and delete entries |
| Shopping lists and items | List and manage lists and items | Not available |
| Cookbooks, foods, units and tools | List and manage | Not available |
| Sharing, households, groups and administration | Multiple management operations | Not available |

The catalogue workflow needs complete paginated recipe listing, full detail,
description and organiser updates, organiser listing and creation, and image
replacement. The legacy untagged and uncategorised summary endpoints were removed;
derive those filters from the supported recipe responses. Follow-on work covers
the remaining recipe, category and tag operations in the table; other API families
are outside this effort.
