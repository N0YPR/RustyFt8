# Tracing Quick Start

Tracing has been set up throughout the project. Here's how to use it:

## Running Tests with Different Log Levels

```bash
# Default (warnings only) - quiet output
cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Show high-level progress (info level)
RUST_LOG=rustyft8::sync::coarse=info cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Show detailed debug info
RUST_LOG=rustyft8::sync::coarse=debug cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Show extremely detailed trace info
RUST_LOG=rustyft8::sync::coarse=trace cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture

# Debug specific frequencies (e.g., 2733 Hz)
RUST_LOG=rustyft8::sync::coarse=trace cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture 2>&1 | grep 2733
```

## What You'll See

### Info Level
- High-level completion messages
- Candidate counts
- Function entry/exit (from #[instrument])

Example:
```
INFO coarse_sync{...}: coarse sync complete total_candidates=200 max_candidates=200
```

### Debug Level
- Sync power normalization details
- Top candidate information
- Important decision points

Example:
```
DEBUG coarse_sync{...}: sync power normalization total_candidates=2067 baseline_40th=1.892
DEBUG coarse_sync{...}: top candidate top_freq=2121.006 top_sync=45.3247 top_time=2.18
```

### Trace Level
- Per-frequency sync2d values
- Parabolic interpolation details
- Frequency normalization values
- Candidate filtering decisions

Example:
```
TRACE coarse_sync{...}: sync2d debug frequency freq=2733 bin=932 max_sync=4.446 ...
TRACE coarse_sync{...}: 2733 Hz candidate PASSED sync=1.550 time=0.12
```

## Multiple Modules

```bash
# Debug all sync modules
RUST_LOG=rustyft8::sync=debug cargo test -- --nocapture

# Mix levels: info for most, trace for coarse sync
RUST_LOG=rustyft8=info,rustyft8::sync::coarse=trace cargo test -- --nocapture

# Everything at trace (very verbose!)
RUST_LOG=rustyft8=trace cargo test -- --nocapture
```

## Filtering Output

```bash
# Use grep to find specific information
RUST_LOG=trace cargo test -- --nocapture 2>&1 | grep "2733 Hz"
RUST_LOG=trace cargo test -- --nocapture 2>&1 | grep "normalization"
RUST_LOG=trace cargo test -- --nocapture 2>&1 | grep "top candidate"
```

## What Was Migrated

The following debug output has been converted to tracing:

### [src/sync/coarse.rs](../src/sync/coarse.rs)
- ✅ Function instrumentation with #[instrument]
- ✅ sync2d debug frequency output (TRACE level)
- ✅ Parabolic interpolation debug (TRACE level)
- ✅ Sync power normalization (DEBUG level)
- ✅ Frequency normalization details (TRACE level)
- ✅ 2733 Hz candidate filtering (TRACE level)
- ✅ Top candidate information (DEBUG level)
- ✅ Coarse sync completion (INFO level)

All the old `eprintln!` and `_debug_sync2d` flags have been replaced with proper structured tracing.

## For More Details

See [TRACING.md](TRACING.md) for:
- Complete API documentation
- Best practices
- How to add tracing to new code
- Performance considerations
- Troubleshooting tips
