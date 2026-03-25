use super::common;
use super::HookContext;
use anyhow::Result;

pub async fn run(ctx: &HookContext<'_>) -> Result<()> {
    common::ensure_data_dir(ctx)?;
    tracing::info!(
        "{}: ready. Login at https://{}  (admin credentials in vault)",
        ctx.instance.name,
        ctx.instance.service_domain
    );
    Ok(())
}
