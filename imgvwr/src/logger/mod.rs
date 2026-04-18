#[cfg(test)]
mod tests;

#[cfg(feature = "logging")]
pub(crate) fn init(level: &str) {
    use tracing_subscriber::EnvFilter;

    // When RUST_LOG is not set, use the configured level for our crates but
    // suppress wgpu/naga/vulkan internals (they spam debug/trace every frame).
    let default_directives = format!("{level},wgpu_core=warn,wgpu_hal=warn,naga=warn");
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&default_directives));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg(not(feature = "logging"))]
pub(crate) fn init(_level: &str) {}
