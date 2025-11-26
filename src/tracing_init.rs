//! Tracing initialization for tests and binaries
//!
//! Provides centralized tracing setup with environment-based filtering.

#[cfg(test)]
use once_cell::sync::Lazy;

/// Initialize tracing for tests with environment-based filtering
///
/// Uses RUST_LOG environment variable to control output:
/// - `RUST_LOG=rustyft8=debug` - Show all debug output
/// - `RUST_LOG=rustyft8::sync::coarse=trace` - Trace specific module
/// - `RUST_LOG=rustyft8=debug,rustyft8::sync=trace` - Mixed levels
///
/// Call this once at the start of each test that needs tracing.
/// Multiple calls are safe (uses once_cell).
#[cfg(test)]
pub fn init_test_tracing() {
    static TRACING: Lazy<()> = Lazy::new(|| {
        use tracing_subscriber::{fmt, EnvFilter};

        // Try to read RUST_LOG, fall back to "rustyft8=warn" if not set
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("rustyft8=warn"));

        fmt()
            .with_env_filter(filter)
            .with_target(true)           // Show module path
            .with_thread_ids(false)      // Usually not needed for tests
            .with_line_number(true)      // Show source line
            .with_test_writer()          // Capture test output
            .init();
    });

    // Force initialization
    Lazy::force(&TRACING);
}

/// Initialize tracing for binaries with environment-based filtering
///
/// Call this early in main() to enable tracing throughout the application.
pub fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rustyft8=info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();
}
