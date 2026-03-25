// FreeSynergy.Node – Bootstrap Installer
//
// Replaces fs-install.sh.  Distributed as a pre-built binary; bootstrapped via:
//
//   curl -fsSL https://install.freesynergy.net/fs-installer -o fs-installer
//   chmod +x fs-installer && ./fs-installer
//
// Flags mirror the old shell script:
//   --repo URL       FSN repository to clone  (default: official GitHub)
//   --target DIR     Installation directory    (default: ~/FreeSynergy.Node)
//   --skip-build     Download pre-built binary instead of compiling
//   --skip-init      Clone + build only; skip `fsn init`

use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use clap::Parser;

// ── CLI args ──────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "fs-installer",
    about = "FreeSynergy.Node – bootstrap installer"
)]
struct Args {
    /// FSN repository URL to clone
    #[arg(long, default_value = "https://github.com/FreeSynergy/Node")]
    repo: String,

    /// Installation directory
    #[arg(long)]
    target: Option<PathBuf>,

    /// Download a pre-built `fsn` binary instead of compiling from source
    #[arg(long)]
    skip_build: bool,

    /// Clone + build only; skip `fsn init`
    #[arg(long)]
    skip_init: bool,
}

// ── Log ───────────────────────────────────────────────────────────────────────

struct Log;

impl Log {
    fn info(msg: &str) {
        eprintln!("\x1b[1;34m==> \x1b[0m{msg}");
    }
    fn ok(msg: &str) {
        eprintln!("\x1b[1;32m✓   \x1b[0m{msg}");
    }
    fn warn(msg: &str) {
        eprintln!("\x1b[1;33m!   \x1b[0m{msg}");
    }
}

// ── Installer ─────────────────────────────────────────────────────────────────

struct Installer {
    repo: String,
    target: PathBuf,
    bin_path: PathBuf,
    skip_build: bool,
    skip_init: bool,
}

impl Installer {
    fn new(args: Args) -> Self {
        let home = Self::home();
        Self {
            target: args.target.unwrap_or_else(|| home.join("FreeSynergy.Node")),
            bin_path: home.join(".local").join("bin").join("fsn"),
            repo: args.repo,
            skip_build: args.skip_build,
            skip_init: args.skip_init,
        }
    }

    async fn run(&self) -> Result<()> {
        let os = Self::detect_os();
        Log::info(&format!("Detected OS: {os}"));

        Self::install_deps(&os)?;
        Self::enable_lingering();
        self.ensure_repo()?;

        if self.skip_build {
            self.download_binary()?;
        } else {
            self.build_binary()?;
        }

        // Ensure ~/.local/bin is on PATH for the fsn init call
        let local_bin = Self::home().join(".local").join("bin");
        let old_path = env::var("PATH").unwrap_or_default();
        env::set_var("PATH", format!("{}:{old_path}", local_bin.display()));

        match Command::new(&self.bin_path).arg("--version").output() {
            Ok(out) => {
                let ver = String::from_utf8_lossy(&out.stdout);
                Log::ok(&format!("fsn {} ready.", ver.trim()));
            }
            Err(e) => bail!("installed fsn binary not executable: {e}"),
        }

        if !self.skip_init {
            Log::info("Starting setup wizard…");
            let status = Command::new(&self.bin_path)
                .arg("init")
                .arg("--root")
                .arg(&self.target)
                .status()
                .context("running fsn init")?;
            if !status.success() {
                bail!("fsn init exited with {status}");
            }
        }

        Ok(())
    }

    fn detect_os() -> String {
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(id) = line.strip_prefix("ID=") {
                    return id.trim_matches('"').to_string();
                }
            }
        }
        "unknown".to_string()
    }

    fn install_deps(os: &str) -> Result<()> {
        let missing: Vec<&str> = ["git", "curl", "podman"]
            .into_iter()
            .filter(|cmd| Self::which(cmd).is_none())
            .collect();

        if !std::path::Path::new("/run/systemd/system").exists() {
            bail!("systemd is required but not running. FSN uses Podman Quadlets (systemd user units).");
        }

        if missing.is_empty() {
            Log::ok("All system dependencies present.");
            return Ok(());
        }

        Log::info(&format!(
            "Installing missing packages: {}",
            missing.join(" ")
        ));
        let pkgs = missing.join(" ");

        let result = match os {
            "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => {
                Self::run_cmd("sudo", &["dnf", "install", "-y", &pkgs])
            }
            "debian" | "ubuntu" | "linuxmint" | "pop" => {
                Self::run_cmd("sudo", &["apt-get", "install", "-y", &pkgs])
            }
            "arch" | "manjaro" => Self::run_cmd("sudo", &["pacman", "-Sy", "--noconfirm", &pkgs]),
            os if os.starts_with("opensuse") || os == "sles" => {
                Self::run_cmd("sudo", &["zypper", "install", "-y", &pkgs])
            }
            _ => {
                Log::warn(&format!("Unknown OS – please install manually: {pkgs}"));
                return Ok(());
            }
        };

        result.with_context(|| format!("installing packages: {pkgs}"))
    }

    fn enable_lingering() {
        if Self::which("loginctl").is_some() {
            let user = env::var("USER").unwrap_or_default();
            if !user.is_empty() {
                Log::info("Enabling systemd user lingering…");
                let _ = Command::new("loginctl")
                    .args(["enable-linger", &user])
                    .stderr(Stdio::null())
                    .status();
            }
        }
    }

    fn ensure_repo(&self) -> Result<()> {
        let target = &self.target;
        if target.join(".git").exists() {
            Log::info(&format!("Updating existing repo at {}", target.display()));
            Self::run_cmd(
                "git",
                &["-C", &target.to_string_lossy(), "pull", "--ff-only"],
            )
        } else {
            Log::info(&format!("Cloning FreeSynergy.Node to {}", target.display()));
            std::fs::create_dir_all(target.parent().unwrap_or(target))?;
            Self::run_cmd(
                "git",
                &[
                    "clone",
                    "--depth",
                    "1",
                    &self.repo,
                    &target.to_string_lossy(),
                ],
            )
        }
    }

    fn build_binary(&self) -> Result<()> {
        if Self::which("cargo").is_none() {
            Log::info("Installing Rust toolchain via rustup…");
            let sh = Self::fetch("https://sh.rustup.rs")?;
            let tmp = std::env::temp_dir().join("rustup-init.sh");
            std::fs::write(&tmp, sh)?;
            Self::run_cmd(
                "sh",
                &[&tmp.to_string_lossy(), "--", "-y", "--profile", "minimal"],
            )
            .context("installing rustup")?;

            let cargo_bin = Self::home().join(".cargo").join("bin");
            let old_path = env::var("PATH").unwrap_or_default();
            env::set_var("PATH", format!("{}:{old_path}", cargo_bin.display()));
        }

        Log::info("Building fsn binary (this may take a few minutes on first run)…");
        let cli_dir = self.target.join("cli");
        Self::run_cmd(
            "cargo",
            &[
                "build",
                "--release",
                "-p",
                "fs-node-cli",
                "--manifest-path",
                &cli_dir.join("Cargo.toml").to_string_lossy(),
            ],
        )
        .context("cargo build")?;

        let built = cli_dir.join("target").join("release").join("fsn");
        std::fs::create_dir_all(self.bin_path.parent().unwrap_or(&self.bin_path))?;
        std::fs::copy(&built, &self.bin_path)
            .with_context(|| format!("copying fsn binary to {}", self.bin_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.bin_path, std::fs::Permissions::from_mode(0o755))?;
        }

        Log::ok(&format!("Installed fsn to {}", self.bin_path.display()));
        Ok(())
    }

    fn download_binary(&self) -> Result<()> {
        let arch = std::env::consts::ARCH;
        let url = format!(
            "{}/releases/latest/download/fs-{arch}-unknown-linux-musl",
            self.repo
        );
        Log::info(&format!("Downloading pre-built fsn binary from {url}…"));

        let bytes = Self::fetch(&url)?;
        std::fs::create_dir_all(self.bin_path.parent().unwrap_or(&self.bin_path))?;
        std::fs::write(&self.bin_path, bytes)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.bin_path, std::fs::Permissions::from_mode(0o755))?;
        }

        Log::ok(&format!("Downloaded fsn to {}", self.bin_path.display()));
        Ok(())
    }

    fn which(cmd: &str) -> Option<PathBuf> {
        env::var_os("PATH").and_then(|paths| {
            env::split_paths(&paths).find_map(|dir| {
                let full = dir.join(cmd);
                full.is_file().then_some(full)
            })
        })
    }

    fn run_cmd(prog: &str, args: &[&str]) -> Result<()> {
        let status = Command::new(prog)
            .args(args)
            .status()
            .with_context(|| format!("running {prog}"))?;
        anyhow::ensure!(status.success(), "{prog} exited with {status}");
        Ok(())
    }

    fn fetch(url: &str) -> Result<Vec<u8>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let client = reqwest::Client::builder().https_only(true).build()?;
                let resp = client.get(url).send().await?.error_for_status()?;
                Ok(resp.bytes().await?.to_vec())
            })
        })
    }

    fn home() -> PathBuf {
        env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Installer::new(Args::parse()).run().await
}
