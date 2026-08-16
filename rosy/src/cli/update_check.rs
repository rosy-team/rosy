use semver::Version;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const GITHUB_API_URL: &str = "https://api.github.com/repos/rosy-team/rosy/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
}

pub struct UpdateHandle {
    rx: mpsc::Receiver<Option<String>>,
}

/// Spawn a background thread that checks for a newer release on GitHub.
/// Returns a handle you can later call `.check()` on to print a warning if needed.
pub fn spawn_update_check() -> UpdateHandle {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let result = check_latest_version();
        let _ = tx.send(result);
    });

    UpdateHandle { rx }
}

impl UpdateHandle {
    /// Non-blocking: if the background check finished and found a newer version,
    /// print an update warning to stderr. Otherwise does nothing.
    pub fn finish(&self) {
        if let Ok(Some(msg)) = self.rx.recv_timeout(Duration::from_millis(100)) {
            eprintln!("{msg}");
        }
    }
}

fn check_latest_version() -> Option<String> {
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build(),
    );
    let response: GithubRelease = agent
        .get(GITHUB_API_URL)
        .header("User-Agent", "rosy-transpiler")
        .header("Accept", "application/vnd.github.v3+json")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;

    let remote_tag = response
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&response.tag_name);
    let remote_version = Version::parse(remote_tag).ok()?;
    let local_version = Version::parse(CURRENT_VERSION).ok()?;

    if remote_version > local_version {
        let release_url =
            format!("https://github.com/rosy-team/rosy/releases/tag/v{remote_version}");
        let changelog_link = format!("\x1b]8;;{release_url}\x1b\\changelog\x1b]8;;\x1b\\");
        Some(format!(
            "\n\x1b[33mA newer version of Rosy is available: v{remote_version} (current: v{local_version}) — {changelog_link}\n\
             \n\
             To update (built from source):\n\
             \x1b[0m  git pull && cargo install --path rosy\n\
             \n\
             \x1b[33mOr download a prebuilt binary from:\n\
             \x1b[0m  {release_url}\n"
        ))
    } else {
        None
    }
}
