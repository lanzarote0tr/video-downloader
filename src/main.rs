mod chromium;
mod config;
mod devtools;
mod devtools_handler;
mod recorder;
mod response_writer;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::Config::new()?;
    println!("Starting capture run {}", cfg.run_id);
    tokio::fs::create_dir_all(&cfg.capture_dir).await?;
    let recorder = recorder::Recorder::new(&cfg.capture_dir.join("traffic.ndjson")).await?;

    let chromium_exe = chromium::ensure_chromium(&cfg.binaries_dir).await?;
    println!(
        "Chromium ready at {} (debug port {})",
        chromium_exe.display(),
        cfg.port
    );

    let mut child = chromium::launch_chromium(&chromium_exe, cfg.port, &cfg.profile_dir).await?;
    let capture_task = tokio::spawn(devtools::capture(cfg.port, recorder.clone()));
    let status = child.wait().await?;
    if !status.success() {
        eprintln!("Chromium exited with status {:?}", status.code());
    }
    capture_task.await??;
    println!("Saved traffic to {}", cfg.capture_dir.display());
    Ok(())
}
