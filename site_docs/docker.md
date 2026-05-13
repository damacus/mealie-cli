# Docker

The release workflow builds and publishes a container image to GitHub Container Registry.

After a release is published, pull the image:

```bash
docker pull ghcr.io/damacus/mealie-cli:latest
```

Run a read-only command:

```bash
docker run --rm \
  -e MEALIE_URL=https://mealie.example.com \
  -e MEALIE_TOKEN=your-token \
  ghcr.io/damacus/mealie-cli:latest \
  recipes search "pesto chicken" --limit 5
```
