# ding

`ding` is a discord bot that periodically reads the WoW Armory and notifies a discord channel when a character has leveled up. Currently only supports WoW: Forever characters.

## Inviting the bot

Replace `<APP_ID>` with your application's client ID (Developer Portal → General Information):

```
https://discord.com/oauth2/authorize?client_id=<APP_ID>&permissions=18432&integration_type=0&scope=bot+applications.commands
```

Scopes: `bot`, `applications.commands`
Bot permissions: Send Messages, Embed Links (`18432`)

## Running with Docker

Images are published to GitHub Container Registry on each release (`linux/amd64` and `linux/arm64`).

```sh
docker run -d --name ding \
  --restart unless-stopped \
  -e DISCORD_TOKEN=<your-bot-token> \
  -v ding-data:/data \
  ghcr.io/martig3/ding:latest
```

- `DISCORD_TOKEN` is required.
- Optionally pass an env file instead of `-e`: `--env-file .env` (but leave `DATABASE_URL` unset or `sqlite:/data/ding.db`).

To build locally: `docker build -t ding .`

### Docker Compose

Put `DISCORD_TOKEN=<your-bot-token>` in a `.env` file next to `docker-compose.yml`, then:

```sh
docker compose up -d --build   # build and start
docker compose logs -f         # follow logs
docker compose down            # stop (data volume is kept)
```

To use the published image instead of building, swap `build: .` for the commented `image:` line in `docker-compose.yml`.
