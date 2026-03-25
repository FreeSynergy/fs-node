// Lifecycle hook executor.
//
// Runs [lifecycle] hooks declared in module TOML files.
//
// Supported actions per phase:
//   on_install      → run, bus_emit
//   on_peer_install → run (triggered when another service is installed)
//   on_update       → backup, run
//   on_swap         → export, run
//   on_decommission → backup, run
//
// All hooks are best-effort: failures are logged as warnings but do not
// abort the deploy. The lifecycle system must never block service start.
//
// # OOP design
//
// `LifecycleHookExt` makes each hook variant responsible for executing
// itself, given a `HookContext`.  `LifecycleRunner::run_phase` no longer
// needs a central `match hook` — it calls `hook.execute(ctx)` directly.
// New hook variants add an arm inside `LifecycleHookExt::execute` only.

use anyhow::Result;
use tracing::{info, warn};

use fs_node_core::config::service::{LifecycleHook, PeerHook};

use super::common::podman_exec;
use super::HookContext;

// ── LifecycleHookExt ──────────────────────────────────────────────────────────

/// Extension trait that gives `LifecycleHook` the ability to execute itself.
///
/// Each variant knows what it does — `LifecycleRunner` only drives the phase
/// loop; it does not contain any hook-specific logic.
#[allow(async_fn_in_trait)]
trait LifecycleHookExt {
    async fn execute(&self, ctx: &HookContext<'_>) -> Result<()>;
}

impl LifecycleHookExt for LifecycleHook {
    async fn execute(&self, ctx: &HookContext<'_>) -> Result<()> {
        match self {
            LifecycleHook::Run { command } => run_shell(ctx, Some(command)).await,
            LifecycleHook::BusEmit { event, .. } => {
                info!(
                    "[lifecycle:bus_emit] {} → event={}",
                    ctx.instance.name, event
                );
                // Bus integration placeholder — bus_emit will be wired in Teil 6
                // when fs-bus is available as a dependency.
                Ok(())
            }
            LifecycleHook::Backup { target } => run_backup(ctx, target.as_deref()).await,
            LifecycleHook::Export { target, format } => {
                run_export(ctx, target.as_deref(), format.as_deref()).await
            }
        }
    }
}

// ── LifecycleRunner ───────────────────────────────────────────────────────────

pub struct LifecycleRunner<'a> {
    ctx: &'a HookContext<'a>,
}

impl<'a> LifecycleRunner<'a> {
    pub fn new(ctx: &'a HookContext<'a>) -> Self {
        Self { ctx }
    }

    pub async fn on_install(&self) -> Result<()> {
        let hooks = self.ctx.instance.class.lifecycle.on_install.clone();
        self.run_phase("on_install", &hooks).await
    }

    /// Called during the Configure phase: after installation, before first start.
    /// Typical use: set admin password, register with IAM, import seed data.
    pub async fn on_configure(&self) -> Result<()> {
        let hooks = self.ctx.instance.class.lifecycle.on_configure.clone();
        self.run_phase("on_configure", &hooks).await
    }

    /// Called during the Migrate phase: schema upgrades, data transformations,
    /// or any preparation needed before swapping to a new service version.
    pub async fn on_migrate(&self) -> Result<()> {
        let hooks = self.ctx.instance.class.lifecycle.on_migrate.clone();
        self.run_phase("on_migrate", &hooks).await
    }

    pub async fn on_update(&self) -> Result<()> {
        let hooks = self.ctx.instance.class.lifecycle.on_update.clone();
        self.run_phase("on_update", &hooks).await
    }

    pub async fn on_decommission(&self) -> Result<()> {
        let hooks = self.ctx.instance.class.lifecycle.on_decommission.clone();
        self.run_phase("on_decommission", &hooks).await
    }

    /// Called when this service is being replaced by a new version.
    pub async fn on_swap(&self) -> Result<()> {
        let hooks = self.ctx.instance.class.lifecycle.on_swap.clone();
        self.run_phase("on_swap", &hooks).await
    }

    /// Run an `on_peer_install` hook (triggered when another service is installed).
    pub async fn peer_hook(&self, hook: &PeerHook) -> Result<()> {
        run_shell(self.ctx, Some(&hook.command)).await
    }

    async fn run_phase(&self, phase: &str, hooks: &[LifecycleHook]) -> Result<()> {
        if hooks.is_empty() {
            return Ok(());
        }
        info!(
            "[lifecycle] {} {}: {} hook(s)",
            self.ctx.instance.name,
            phase,
            hooks.len()
        );
        for hook in hooks {
            if let Err(e) = hook.execute(self.ctx).await {
                warn!(
                    "[lifecycle] {} {} hook failed (continuing): {:#}",
                    self.ctx.instance.name, phase, e
                );
            }
        }
        Ok(())
    }
}

// ── Hook helpers ──────────────────────────────────────────────────────────────
//
// Free functions so `LifecycleHookExt::execute` can call them without
// going through `LifecycleRunner` (which would create a circular reference).

#[allow(clippy::cognitive_complexity)]
async fn run_shell(ctx: &HookContext<'_>, command: Option<&str>) -> Result<()> {
    let cmd = match command {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            warn!("[lifecycle:run] {} has no command", ctx.instance.name);
            return Ok(());
        }
    };

    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let (bin, args) = parts.split_first().unwrap_or((&"", &[]));
    info!("[lifecycle:run] {} exec: {}", ctx.instance.name, cmd);

    let out = podman_exec(&ctx.instance.name, &{
        let mut all = vec![*bin];
        all.extend_from_slice(args);
        all
    })
    .await?;

    if !out.is_empty() {
        info!(
            "[lifecycle:run] {} output: {}",
            ctx.instance.name,
            out.trim()
        );
    }
    Ok(())
}

async fn run_backup(ctx: &HookContext<'_>, target: Option<&str>) -> Result<()> {
    let src = ctx.instance_data_dir();
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dst = target
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| src.with_file_name(format!("{}-backup-{}", ctx.instance.name, ts)));

    info!("[lifecycle:backup] {} → {}", src.display(), dst.display());

    let status = tokio::process::Command::new("cp")
        .args(["-a", &src.to_string_lossy(), &dst.to_string_lossy()])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("backup cp failed for {}", ctx.instance.name);
    }
    Ok(())
}

async fn run_export(
    ctx: &HookContext<'_>,
    target: Option<&str>,
    format: Option<&str>,
) -> Result<()> {
    let fmt = format.unwrap_or("json");
    let out_path = target.map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::PathBuf::from(format!("/tmp/fs-export-{}.{}", ctx.instance.name, fmt))
    });

    info!(
        "[lifecycle:export] {} format={} → {}",
        ctx.instance.name,
        fmt,
        out_path.display()
    );
    // Actual export implementation is service-specific and provided via
    // the `command` field in the hook (run_shell handles the exec).
    // This stub logs intent and returns OK for Bus-signalling purposes.
    Ok(())
}
