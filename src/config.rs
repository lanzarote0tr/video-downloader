use anyhow::{Context, Result};
use chrono::Utc;
use std::{env, net::TcpListener, path::PathBuf};

pub struct Config {
    pub port: u16,
    pub run_id: String,
    pub binaries_dir: PathBuf,
    pub capture_dir: PathBuf,
    pub profile_dir: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self> {
        let cwd = env::current_dir().context("resolve current directory")?;
        let run_id = Utc::now().format("run-%Y%m%d-%H%M%S").to_string();
        let binaries_dir = cwd.join("binaries");
        let capture_dir = cwd.join("captures").join(&run_id);
        let profile_dir = cwd.join("profiles").join(&run_id);
        let port = pick_port()?;
        Ok(Self {
            port,
            run_id,
            binaries_dir,
            capture_dir,
            profile_dir,
        })
    }
}

fn pick_port() -> Result<u16> {
    let socket = TcpListener::bind(("127.0.0.1", 0)).context("allocate local port")?;
    let port = socket.local_addr()?.port();
    drop(socket);
    Ok(port)
}
