// Integration test: parse all modules from the Store.
//
// Loads every .toml in FreeSynergy.Store/packages/ via ServiceRegistry
// and asserts that key container modules are present and well-formed.
//
// Layout after store restructuring (2026-03-23):
//   packages/apps/node/{name}/manifest.toml   -- native apps (new format)
//   packages/containers/{name}/{name}.toml     -- containers ([module] format, ServiceRegistry ready)
//   packages/apps/node/zentinel/providers/     -- DNS/ACME providers (per tool, not global)
//
// The test is skipped gracefully when the store directory does not exist
// (e.g. in CI without a checked-out Store repo).

use std::path::PathBuf;

use fs_node_core::config::plugin::PluginConfig;
use fs_node_core::config::registry::ServiceRegistry;

fn store_packages_dir() -> PathBuf {
    // From cli/crates/fs-node-core/ go up 4 levels -> /home/kal/Server/
    // then into fs-store/packages/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../fs-store/packages")
}

fn zentinel_providers_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../fs-store/packages/apps/node/zentinel/providers")
}

#[test]
fn all_store_modules_parse_without_error() {
    let dir = store_packages_dir();
    if !dir.exists() {
        eprintln!("SKIP: Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");
    let classes: Vec<_> = registry.all().collect();

    assert!(
        !classes.is_empty(),
        "expected at least one module to be loaded"
    );
    eprintln!("Loaded {} module classes", classes.len());
}

#[test]
fn expected_modules_are_present() {
    let dir = store_packages_dir();
    if !dir.exists() {
        eprintln!("SKIP: Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");

    // Native apps (apps/node/*) use manifest.toml format -- ServiceRegistry skips them.
    // Only containers still use {name}.toml format that ServiceRegistry can parse.
    let required = [
        "containers/forgejo",
        "containers/outline",
        "containers/postgres",
        "containers/dragonfly",
        "containers/openobserver",
    ];

    for key in &required {
        assert!(
            registry.get(key).is_some(),
            "expected module '{key}' not found in registry"
        );
    }
}

#[test]
fn all_container_modules_have_image() {
    let dir = store_packages_dir();
    if !dir.exists() {
        eprintln!("SKIP: Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");

    for (key, class) in registry.all() {
        // Native apps have no container block — skip them.
        let Some(container) = &class.container else {
            continue;
        };

        assert!(
            !container.image.is_empty(),
            "module '{key}' has empty container.image"
        );
        assert!(
            !container.image_tag.is_empty(),
            "module '{key}' has empty container.image_tag"
        );
    }
}

#[test]
fn all_container_modules_have_healthcheck() {
    let dir = store_packages_dir();
    if !dir.exists() {
        eprintln!("SKIP: Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");

    for (key, class) in registry.all() {
        // Native apps have no container block — skip them.
        let Some(container) = &class.container else {
            continue;
        };

        assert!(
            container.healthcheck.is_some(),
            "container module '{key}' is missing container.healthcheck (required by convention)"
        );
    }
}

// Providers are per-tool (no longer global plugins).
// Zentinel carries its own providers/ directory.
#[test]
fn zentinel_dns_and_acme_providers_parse() {
    let providers_dir = zentinel_providers_dir();
    if !providers_dir.exists() {
        eprintln!(
            "SKIP: zentinel providers dir not found at {}",
            providers_dir.display()
        );
        return;
    }

    let required = [
        ("dns", "hetzner"),
        ("dns", "cloudflare"),
        ("acme", "letsencrypt"),
    ];

    for (kind, name) in &required {
        let path = providers_dir.join(kind).join(format!("{name}.toml"));
        assert!(path.exists(), "provider file missing: {}", path.display());
        let content = std::fs::read_to_string(&path).expect("read provider toml");
        let _: PluginConfig = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("parse error in {}/{}.toml: {e}", kind, name));
    }
}
