use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{fs, io::AsyncWriteExt, process::Command, task};

const SNAPSHOT_BASE: &str = "https://storage.googleapis.com/chromium-browser-snapshots";

struct Platform {
    tag: &'static str,
    zip: &'static str,
    exe: PathBuf,
}

impl Platform {
    fn current() -> Self {
        let arch = env::consts::ARCH;
        match env::consts::OS {
            "macos" if arch == "aarch64" => Platform {
                tag: "Mac_Arm",
                zip: "chrome-mac.zip",
                exe: PathBuf::from("chrome-mac/Chromium.app/Contents/MacOS/Chromium"),
            },
            "macos" => Platform {
                tag: "Mac",
                zip: "chrome-mac.zip",
                exe: PathBuf::from("chrome-mac/Chromium.app/Contents/MacOS/Chromium"),
            },
            "windows" => Platform {
                tag: "Win_x64",
                zip: "chrome-win.zip",
                exe: PathBuf::from("chrome-win/chrome.exe"),
            },
            _ if arch == "aarch64" || arch == "arm" => Platform {
                tag: "Linux_Arm64",
                zip: "chrome-linux.zip",
                exe: PathBuf::from("chrome-linux/chrome"),
            },
            _ => Platform {
                tag: "Linux_x64",
                zip: "chrome-linux.zip",
                exe: PathBuf::from("chrome-linux/chrome"),
            },
        }
    }
}

pub async fn ensure_chromium(root: &Path) -> Result<PathBuf> {
    let platform = Platform::current();
    let platform_dir = root.join(platform.tag);
    let exe_path = platform_dir.join(&platform.exe);
    if exe_path.exists() {
        return Ok(exe_path);
    }

    fs::create_dir_all(&platform_dir)
        .await
        .context("create platform dir")?;
    download_and_extract(&platform, &platform_dir).await?;
    if exe_path.exists() {
        Ok(exe_path)
    } else {
        Err(anyhow!("chromium executable missing after download"))
    }
}

async fn download_and_extract(platform: &Platform, dest: &Path) -> Result<()> {
    let client = Client::builder().build().context("build http client")?;
    let revision_url = format!("{}/{}/LAST_CHANGE", SNAPSHOT_BASE, platform.tag);
    let revision = client
        .get(revision_url)
        .send()
        .await
        .context("fetch latest revision")?
        .error_for_status()?
        .text()
        .await?
        .trim()
        .to_string();

    let zip_url = format!(
        "{}/{}/{}/{}",
        SNAPSHOT_BASE, platform.tag, revision, platform.zip
    );
    let zip_path = dest.join("chromium.zip");
    let mut resp = client
        .get(zip_url)
        .send()
        .await
        .context("download chromium archive")?
        .error_for_status()?;
    let mut file = fs::File::create(&zip_path)
        .await
        .context("create archive file")?;
    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
    }
    file.flush().await?;

    let dest_owned = dest.to_path_buf();
    let zip_owned = zip_path.clone();
    task::spawn_blocking(move || -> Result<()> {
        let reader = std::fs::File::open(&zip_owned).context("open downloaded archive")?;
        let mut archive = zip::ZipArchive::new(reader).context("read zip archive")?;
        archive
            .extract(&dest_owned)
            .context("extract chromium archive")?;
        std::fs::remove_file(&zip_owned).ok();
        Ok(())
    })
    .await??;
    Ok(())
}

pub async fn launch_chromium(
    exe: &Path,
    port: u16,
    profile_dir: &Path,
) -> Result<tokio::process::Child> {
    fs::create_dir_all(profile_dir)
        .await
        .context("create profile dir")?;
    let mut cmd = Command::new(exe);
    cmd.arg(format!("--remote-debugging-port={}", port))
        .arg(format!("--user-data-dir={}", profile_dir.to_string_lossy()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-popup-blocking")
        .arg("--disable-background-networking")
        .arg("--use-mock-keychain")
        .arg("--password-store=basic")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    cmd.spawn().context("launch chromium")
}
