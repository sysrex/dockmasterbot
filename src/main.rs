use anyhow::{Context, Result};
use clap::Parser;
use octocrab::models;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf, time::Duration};
use tokio::time::sleep;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug, Clone)]
#[command(name = "github-tag-watcher", author, version, about)]
struct Args {
    /// Comma-separated list like: owner1/repo1,owner2/repo2
    #[arg(long, env = "REPOS")]
    repos: String,

    /// Poll interval in seconds
    #[arg(long, env = "POLL_SECS", default_value = "120")]
    poll_secs: u64,

    /// GitHub token (PAT). Optional but recommended.
    #[arg(long, env = "GITHUB_TOKEN")]
    github_token: Option<String>,

    /// Telegram bot token (e.g., 123456:ABC-DEF...)
    #[arg(long, env = "TG_BOT_TOKEN")]
    tg_bot_token: String,

    /// Telegram chat id (e.g., -1001234567890 for channels/supergroups)
    #[arg(long, env = "TG_CHAT_ID", allow_hyphen_values = true)]
    tg_chat_id: i64,

    /// Path to state file
    #[arg(long, env = "STATE_PATH", default_value = "state.json")]
    state_path: PathBuf,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct State {
    /// repo -> last_seen_tag
    last_seen: HashMap<String, String>,
}

impl State {
    fn load(p: &PathBuf) -> Result<Self> {
        if p.exists() {
            let s = fs::read_to_string(p)
                .with_context(|| format!("reading state file {}", p.display()))?;
            Ok(serde_json::from_str(&s).context("parsing state json")?)
        } else {
            Ok(Self::default())
        }
    }
    fn save(&self, p: &PathBuf) -> Result<()> {
        let tmp = format!("{}.tmp", p.display());
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("writing tmp state {}", tmp))?;
        fs::rename(&tmp, p).with_context(|| format!("replacing state {}", p.display()))?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let args = Args::parse();
    info!("starting with repos: {}", args.repos);

    let mut state = State::load(&args.state_path).unwrap_or_default();

    let octo = if let Some(token) = &args.github_token {
        octocrab::OctocrabBuilder::new()
            .personal_token(token.clone())
            .build()?
    } else {
        octocrab::Octocrab::builder().build()?
    };

    let repos: Vec<String> = args
        .repos
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    loop {
        for repo in &repos {
            if let Err(e) = check_repo(repo, &octo, &mut state, &args).await {
                error!(%repo, error=?e, "repo check failed");
            }
        }
        // Persist state after each full pass
        if let Err(e) = state.save(&args.state_path) {
            error!(error=?e, "state save failed");
        }
        sleep(Duration::from_secs(args.poll_secs)).await;
    }
}

async fn check_repo(
    repo: &str,
    octo: &octocrab::Octocrab,
    state: &mut State,
    args: &Args,
) -> Result<()> {
    let (owner, name) = repo
        .split_once('/')
        .context("repo must be owner/repo")?;

    // Strategy: prefer latest release (if any), else latest tag.
    let latest = match latest_release_tag(octo, owner, name).await {
        Ok(tag) => tag,
        Err(_) => latest_raw_tag(octo, owner, name).await?,
    };

    let last_seen = state.last_seen.get(repo).cloned();
    match last_seen {
        Some(ref t) if t == &latest => {
            // no change
            Ok(())
        }
        _ => {
            info!(%repo, %latest, "new tag detected");
            notify_telegram(
                &args.tg_bot_token,
                args.tg_chat_id,
                format!(
                    "🚀 New tag in *{repo}*: `{latest}`\nhttps://github.com/{repo}/releases/tag/{latest}"
                ),
            )
                .await?;
            state.last_seen.insert(repo.to_string(), latest);
            Ok(())
        }
    }
}

async fn latest_release_tag(
    octo: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
) -> Result<String> {
    // list releases: newest first by creation date
    let releases: Vec<models::repos::Release> = octo
        .repos(owner, repo)
        .releases()
        .list()
        .per_page(1)
        .send()
        .await?
        .items;

    let tag = releases
        .first()
        .map(|r| r.tag_name.clone())
        .context("no releases found with tag_name")?;

    Ok(tag)
}

async fn latest_raw_tag(
    octo: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
) -> Result<String> {
    // list tags: GitHub returns most recent commit/tag first
    // (Note: this is not strictly guaranteed to be semver-highest)
    let tags = octo
        .repos(owner, repo)
        .list_tags()
        .per_page(1)
        .send()
        .await?
        .items;

    let tag = tags
        .first()
        .map(|t| t.name.clone())
        .context("no raw tags found")?;

    Ok(tag)
}

async fn notify_telegram(bot_token: &str, chat_id: i64, text: String) -> Result<()> {
    // Telegram expects MarkdownV2 or HTML – we use MarkdownV2-safe escaping for backticks
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "Markdown"
    });

    let resp = client.post(url).json(&payload).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("telegram send failed: {} body={}", status, body);
    }
    Ok(())
}