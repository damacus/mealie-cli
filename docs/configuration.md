# Configuration

`mealie-cli` reads configuration from environment variables.

```bash
export MEALIE_URL=https://mealie.example.com
export MEALIE_TOKEN=your-token
```

## Environment Variables

- `MEALIE_URL`: required Mealie base URL. A trailing slash is allowed.
- `MEALIE_TOKEN`: required bearer token.
- `USE_INSECURE_HTTP`: optional. Set to `yes` to allow an `http://`
  `MEALIE_URL`; HTTPS is required by default.

Only use the HTTP override for a deliberately isolated local or private
network. It permits the bearer token to travel without transport encryption.

Requests use:

```text
Authorization: Bearer <token>
```

The token is never printed by the CLI.
