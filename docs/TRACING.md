# Tracing and Observability Guide

This project uses the `tracing` crate for structured logging and observability. This provides much better debugging capabilities than `println!` or `eprintln!`.

## Quick Start

### Running Tests with Tracing

Control tracing output via the `RUST_LOG` environment variable:

```bash
# Show only warnings (default)
cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Show all debug output from rustyft8
RUST_LOG=rustyft8=debug cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Show trace-level output from coarse sync module only
RUST_LOG=rustyft8::sync::coarse=trace cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Show debug for all sync modules, trace for coarse sync
RUST_LOG=rustyft8::sync=debug,rustyft8::sync::coarse=trace cargo test -- --ignored --nocapture

# Show everything at trace level (very verbose!)
RUST_LOG=rustyft8=trace cargo test -- --nocapture
```

### Log Levels

From least to most verbose:
- `error` - Critical errors only
- `warn` - Warnings (default for tests)
- `info` - High-level progress information
- `debug` - Detailed debugging information
- `trace` - Very detailed trace information

## Using Tracing in Code

### Basic Logging

```rust
use tracing::{error, warn, info, debug, trace};

pub fn my_function() {
    info!("Starting processing");
    debug!("Processing item with id={}", item_id);
    trace!(value = %some_value, "Detailed trace info");
}
```

### Structured Fields

Use key-value pairs for better filtering:

```rust
use tracing::info;

info!(
    candidates = candidates.len(),
    sync_min = %sync_min,
    "Found candidates"
);
```

### Instrumentation

Add `#[instrument]` to automatically trace function entry/exit:

```rust
use tracing::instrument;

#[instrument(skip(signal), fields(signal_len = signal.len()))]
pub fn process_signal(signal: &[f32], threshold: f32) -> Result<Vec<Data>> {
    // Function parameters are automatically logged on entry
    // Return value is logged on exit
    Ok(vec![])
}
```

Options for `#[instrument]`:
- `skip(arg)` - Don't log this argument (e.g., large arrays)
- `fields(key = value)` - Add custom fields
- `level = "debug"` - Set logging level (default: info)

### Conditional Debug Code

Only execute expensive debug code when tracing is enabled:

```rust
use tracing::{trace, enabled, Level};

if enabled!(Level::TRACE) {
    // This block only runs if TRACE level is enabled
    let expensive_debug_info = compute_stats(&data);
    trace!(
        mean = %expensive_debug_info.mean,
        stddev = %expensive_debug_info.stddev,
        "Computed statistics"
    );
}
```

## Examples from This Project

### Coarse Sync Module

The coarse sync module demonstrates various tracing patterns:

```rust
// Function-level instrumentation
#[instrument(skip(signal), fields(signal_len = signal.len()))]
pub fn coarse_sync(signal: &[f32], freq_min: f32, freq_max: f32) -> Result<Vec<Candidate>> {
    // High-level info
    info!(
        total_candidates = candidates.len(),
        "coarse sync complete"
    );

    // Debug info for development
    debug!(
        baseline_40th = %baseline,
        "sync power normalization"
    );

    // Detailed trace only when needed
    if enabled!(Level::TRACE) {
        trace!(
            freq = %freq,
            sync_power = %sync_raw,
            "processing candidate"
        );
    }

    Ok(candidates)
}
```

### Usage Examples

```bash
# See high-level coarse sync progress
RUST_LOG=rustyft8::sync::coarse=info cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Debug sync power normalization issues
RUST_LOG=rustyft8::sync::coarse=debug cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Trace specific frequencies (e.g., 2733 Hz)
RUST_LOG=rustyft8::sync::coarse=trace cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture 2>&1 | grep 2733

# Debug sync2d computation
RUST_LOG=rustyft8::sync::spectra=debug,rustyft8::sync::coarse=debug cargo test -- --nocapture
```

## Best Practices

### DO ✅

- Use `info!` for high-level progress (e.g., "Starting decode pass 1")
- Use `debug!` for important values and decisions (e.g., "Found 42 candidates")
- Use `trace!` for detailed iteration data (e.g., per-frequency sync values)
- Use structured fields: `debug!(count = items.len(), "Processing")` not `debug!("Processing {} items", items.len())`
- Use `#[instrument]` on public API functions
- Use `skip(large_array)` to avoid logging huge data structures
- Use `enabled!()` guard for expensive debug computations

### DON'T ❌

- Don't use `println!` or `eprintln!` in library code (tests are OK)
- Don't log at `trace` level inside tight loops without `enabled!()` guard
- Don't include sensitive data in logs
- Don't format strings unless tracing is enabled: use `%value` or `?value`
- Don't add tracing to every single function - focus on key decision points

## Output Format

Tracing output includes:
- Timestamp
- Level (INFO, DEBUG, TRACE)
- Target (module path like `rustyft8::sync::coarse`)
- Line number
- Message and fields

Example:
```
2025-11-26T12:34:56.789Z DEBUG rustyft8::sync::coarse:285: sync power normalization total_candidates=200 baseline_40th=1.234
```

## Performance

Tracing has **zero cost when disabled**:
- Disabled log levels are compiled out (when not using dynamic filtering)
- No overhead in release builds when `RUST_LOG` is not set
- Use `enabled!()` guards for expensive computations

## Integration with Tests

All integration tests should call `init_test_tracing()` at the start:

```rust
#[test]
fn test_my_feature() {
    init_test_tracing();
    // ... test code
}
```

This allows controlling test output with `RUST_LOG` without cluttering test output by default.

## Troubleshooting

### Tracing Not Working

1. Check `RUST_LOG` is set: `echo $RUST_LOG`
2. Use `--nocapture` flag: `cargo test -- --nocapture`
3. Verify module path is correct: `RUST_LOG=rustyft8=trace`

### Too Much Output

1. Increase log level: `RUST_LOG=rustyft8=info` instead of `debug`
2. Filter to specific module: `RUST_LOG=rustyft8::sync::coarse=debug`
3. Grep for specific patterns: `cargo test -- --nocapture 2>&1 | grep "candidate"`

### Not Enough Output

1. Lower log level: `RUST_LOG=rustyft8=trace`
2. Check if code uses `enabled!()` guards - may need to enable trace level
3. Check if `#[instrument]` is at wrong level

## Further Reading

- [tracing documentation](https://docs.rs/tracing/)
- [tracing-subscriber documentation](https://docs.rs/tracing-subscriber/)
- [Tokio tracing guide](https://tokio.rs/tokio/topics/tracing)
