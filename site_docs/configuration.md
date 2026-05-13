# Configuration

`mealie-cli` reads configuration from environment variables.

```bash
export MEALIE_URL=https://mealie.example.com
export MEALIE_TOKEN=your-token
```

## Environment Variables

- `MEALIE_URL`: required Mealie base URL. A trailing slash is allowed.
- `MEALIE_TOKEN`: required bearer token.

Requests use:

```text
Authorization: Bearer <token>
```

The token is never printed by the CLI.
