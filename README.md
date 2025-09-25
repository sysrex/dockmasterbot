# Dockmaster Bot – GitHub Tag Watcher to Telegram

A tiny Rust service that watches one or more GitHub repositories for new tags/releases and posts a message to a Telegram chat/channel.

It’s designed to be simple, lightweight, and container-friendly. You can run it as a standalone binary or in Docker.


## What it does
- Polls the latest GitHub release tag (or, if no releases, the latest raw tag) for each configured repo
- Remembers the last seen tag per repo in a small JSON state file
- Sends a Markdown-formatted message to a Telegram chat/channel when a new tag is detected


## Requirements
- A Telegram bot token (from @BotFather)
- A Telegram chat ID where the bot will send messages
  - For channels/supergroups this is often a negative number like -1001234567890
  - Add the bot to the chat and ensure it has permission to send messages
- Optional: a GitHub token (PAT) to increase API rate limits
- Either Rust (to build from source) or Docker (to run the container)


## Configuration
You can configure the app via CLI flags or environment variables. All flags have corresponding env vars (shown in parentheses).

- --repos (REPOS) [required]
  - Comma-separated list of repositories in owner/repo form
  - Example: "rust-lang/rust,octocat/Hello-World"
- --poll-secs (POLL_SECS) [default: 120]
  - Polling interval in seconds
- --github-token (GITHUB_TOKEN) [optional]
  - GitHub Personal Access Token to raise rate limits
- --tg-bot-token (TG_BOT_TOKEN) [required]
  - Telegram bot token from @BotFather
- --tg-chat-id (TG_CHAT_ID) [required]
  - Telegram chat ID. Negative IDs are supported (e.g., -1001234567890)
  - Note: This app accepts hyphen-starting values as a normal argument (no need to use =). Both of these are fine: --tg-chat-id -100123..., --tg-chat-id=-100123...
- --state-path (STATE_PATH) [default: state.json]
  - Path to the JSON state file persisted on disk


## Example: Run locally
```bash
# Using environment variables
REPOS="rust-lang/rust,octocat/Hello-World" \
TG_BOT_TOKEN="123456:ABC-DEF..." \
TG_CHAT_ID="-1001234567890" \
POLL_SECS=120 \
GITHUB_TOKEN="ghp_..." \  # optional
cargo run --release -- \
  --repos "$REPOS" \
  --tg-bot-token "$TG_BOT_TOKEN" \
  --tg-chat-id $TG_CHAT_ID
```
Notes:
- If you pass the chat ID as a positional value to --tg-chat-id, negative values are accepted without quoting: --tg-chat-id -1001234567890
- Without GITHUB_TOKEN you may hit lower rate limits on the GitHub API.


## Example: Docker
A multi-stage Dockerfile is included. To build and run:

```bash
# Build image
docker build -t dockmasterbot:latest .

# Run container (writes state.json to the current directory)
docker run --rm \
  -e REPOS="rust-lang/rust,octocat/Hello-World" \
  -e TG_BOT_TOKEN="123456:ABC-DEF..." \
  -e TG_CHAT_ID="-1001234567890" \
  -e POLL_SECS=120 \
  -e GITHUB_TOKEN="ghp_..." \  # optional
  -v "$(pwd)/state.json:/app/state.json" \
  --name dockmasterbot \
  dockmasterbot:latest \
  --state-path /app/state.json
```

Tip: Mount a persistent volume for the state file so the bot remembers previously seen tags across restarts.


## Message format
When a new tag is detected, the bot sends a Telegram message like:

```
🚀 New tag in owner/repo: `v1.2.3`
https://github.com/owner/repo/releases/tag/v1.2.3
```

Messages use Telegram’s Markdown parse mode.


## Logging
- Structured logs are printed at info level by default.
- You can control verbosity with RUST_LOG, e.g.:
  - RUST_LOG=debug cargo run -- ...
  - RUST_LOG="dockmasterbot=debug" docker run ...


## State file
- JSON that maps repo => last_seen_tag
- Written atomically each poll cycle (via a temporary file + rename)
- Path is controlled by --state-path / STATE_PATH (default: ./state.json)


## Troubleshooting
- Telegram errors (HTTP 400): ensure the bot is in the chat and chat_id is correct (channels/supergroups often use -100... prefix).
- Permission denied on state file: adjust --state-path to a writable path or mount a volume with correct ownership in Docker.
- GitHub rate limiting: provide --github-token / GITHUB_TOKEN to increase limits.


## Building from source
Requires Rust 1.80+.

```bash
cargo build --release
./target/release/dockmasterbot --help
```

Note: Internally the CLI name is displayed as "github-tag-watcher" in --help, but the binary is dockmasterbot.


## Reference: Flags
```text
--repos <REPOS>                comma-separated owner/repo list (env: REPOS)
--poll-secs <secs>             poll interval in seconds (env: POLL_SECS, default 120)
--github-token <TOKEN>         GitHub token (env: GITHUB_TOKEN)
--tg-bot-token <TOKEN>         Telegram bot token (env: TG_BOT_TOKEN)
--tg-chat-id <ID>              Telegram chat ID (supports negative values) (env: TG_CHAT_ID)
--state-path <PATH>            state file path (env: STATE_PATH, default state.json)
```
