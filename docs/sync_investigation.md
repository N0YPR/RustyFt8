# Sync Module Investigation - Progress Report

## Summary

This document tracks the investigation into achieving 95% match rate between RustyFt8's coarse sync and WSJT-X's reference implementation.

## Current Status (2025-11-26)

### ✅ Achievements

1. **Spectra Computation**: 99.21% match with WSJT-X
   - Fixed NFFT1/NH1 constants (3840/1920 instead of 4096/2048)
   - Fixed WAV file reading (no normalization, matching WSJT-X)
   - Fixed Fortran 1-indexed vs Rust 0-indexed array mapping
   - Created comprehensive test: `tests/sync/test_spectra.rs`
   - Test validates all 713,868 spectra values

2. **Sync2d Computation**: 100% match with WSJT-X (with jstrt=13)
   - Created comprehensive test: `tests/sync/test_sync2d.rs`
   - Test validates sync2d correlation matrix
   - All peak lags match exactly
   - Created Fortran reference generator: `tests/sync/test_sync2d.f90`

3. **Candidate Selection**: 77% match rate (154/200 candidates)
   - Both implementations find 200 candidates
   - Strong signals match perfectly
   - Weak signals (~1.0-1.6 sync power) have mismatches

### 🔍 Critical Discovery: jstrt Paradox

A fundamental inconsistency was discovered in the `jstrt` (start lag) calculation:

#### The Problem

```rust
// Mathematically correct (WSJT-X uses nint() which rounds):
let jstrt = (0.5 / tstep).round() as i32;  // = 13

// Truncated (wrong, but matches test data):
let jstrt = (0.5 / tstep) as i32;  // = 12
```

#### Observed Behavior

| jstrt Value | Sync2d Match | Candidate Match | Notes |
|-------------|--------------|-----------------|-------|
| 12 (truncated) | Offset by 1 lag | 77.0% (154/200) | Wrong math, but matches test data |
| 13 (rounded) | 100% match | 0.5% (1/200) | Correct math, all times 0.04s early |

#### Analysis

**With jstrt=13 (correct):**
- Sync2d values match WSJT-X exactly (100%)
- Peak lags match exactly
- But candidate times are consistently 0.04s (1 time step) early
- Example: WSJT-X finds 2571.9 Hz at 0.300s, we find it at 0.260s

**With jstrt=12 (truncated):**
- Sync2d values are offset by 1 lag from WSJT-X
- But candidate selection somehow compensates and achieves 77% match
- This suggests the test reference data may be inconsistent

#### Verification

Ran WSJT-X's test_sync2d.f90 which computes jstrt:
```
jstrt = nint(0.5/tstep) = nint(12.5) = 13
```

This confirms WSJT-X should use jstrt=13, yet the sync8 output suggests jstrt=12 behavior.

### 📊 Test Infrastructure Created

1. **tests/sync/test_spectra.rs** - Validates FFT/spectra computation
   - Compares 713,868 spectra values
   - Uses CSV reference data from WSJT-X
   - Run with: `cargo test test_spectra_matches_wsjtx -- --ignored`

2. **tests/sync/test_sync2d.rs** - Validates sync2d correlation
   - Compares sync2d values at key frequency bins
   - Validates peak detection
   - Run with: `cargo test test_sync2d_matches_wsjtx -- --ignored`

3. **tests/sync/test_coarse.rs** - Validates full candidate selection
   - Compares 200 candidates against WSJT-X reference
   - Currently at 77% match rate
   - Run with: `cargo test test_coarse_sync_matches_wsjtx -- --ignored`

## Code Changes Made

### src/sync/mod.rs
```rust
/// FFT size for symbol spectra
/// CRITICAL: MUST be 2*NSPS = 3840 to match WSJT-X exactly!
pub const NFFT1: usize = 2 * NSPS; // 3840 = 2 * 1920

/// Number of FFT bins
pub const NH1: usize = NFFT1 / 2; // 1920
```

### src/sync/spectra.rs (line 302)
```rust
// CURRENT (77% match):
let jstrt = (0.5 / (NSTEP as f32 / SAMPLE_RATE)) as i32; // = 12

// SHOULD BE (100% sync2d match, but breaks candidates):
let jstrt = (0.5 / (NSTEP as f32 / SAMPLE_RATE)).round() as i32; // = 13
```

### tests/test_utils.rs
```rust
/// Read WAV file WITHOUT normalization (matches WSJT-X)
pub fn read_wav_file_raw(path: &str) -> Result<Vec<f32>, String> {
    // Convert i16 directly to f32 (no /32768.0 normalization)
    reader.into_samples::<i16>()
        .map(|s| s.map(|v| v as f32))
        .collect()
}
```

## Files Added

- `tests/sync/test_sync2d.rs` - Rust test for sync2d validation
- `tests/sync/test_sync2d.f90` - Fortran reference generator
- `tests/sync/sync2d_ref.csv` - WSJT-X reference sync2d values (1,250 values)
- `tests/sync/spectra.csv` - WSJT-X reference spectra (713,868 values)
- `tests/sync/avg_spectrum.csv` - WSJT-X reference average spectrum (1,920 values)

## Next Steps

### Option A: Fix jstrt and Regenerate Reference Data
1. Use jstrt=13 (correct rounding)
2. Regenerate test_sync8 reference data with verified WSJT-X version
3. Verify all candidates match with corrected data

### Option B: Accept Current State and Document
1. Keep jstrt=12 to maintain 77% match rate
2. Document the jstrt inconsistency as a known issue
3. Focus on other improvements (fine sync, LDPC decoding, etc.)

### Option C: Deep Investigation
1. Examine WSJT-X's sync8.f90 more carefully for hidden lag adjustments
2. Check if WSJT-X has multiple code paths that use different jstrt values
3. Compare against multiple WSJT-X versions to identify when behavior changed

## Debugging Tips

### Enable Trace Logging
```bash
RUST_LOG=rustyft8::sync::spectra=trace cargo test test_coarse -- --ignored --nocapture
```

### Compare Specific Bins
The test_sync2d program outputs sync2d values for key bins:
- Bin 477 (1490.6 Hz) - Weak signal, peak at lag=0
- Bin 823 (2571.9 Hz) - Strong signal, peak at lag=7
- Bin 811 (2534.4 Hz) - Strong signal, peak at lag=59

### Recompile Fortran Tests
```bash
# test_spectra
gfortran -o tests/sync/test_spectra tests/sync/test_spectra.f90 \
  -I wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8 \
  wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/CMakeFiles/wsjt_fort.dir/lib/four2a.f90.o \
  wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/CMakeFiles/wsjt_fort.dir/lib/fftw3mod.f90.o \
  -lfftw3f -lm

# test_sync2d
gfortran -o tests/sync/test_sync2d tests/sync/test_sync2d.f90 \
  -I wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8 \
  wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/CMakeFiles/wsjt_fort.dir/lib/four2a.f90.o \
  wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/CMakeFiles/wsjt_fort.dir/lib/fftw3mod.f90.o \
  -lfftw3f -lm

# test_sync8
gfortran tests/sync/test_sync8.f90 \
  wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/libwsjt_fort.a \
  -o tests/sync/test_sync8 -lfftw3f -lm -O2
```

## References

- WSJT-X sync8.f90: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/sync8.f90`
- FT8 parameters: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8_params.f90`
- RustyFt8 sync: `src/sync/spectra.rs`, `src/sync/coarse.rs`

## Contributors

Investigation conducted by Claude Code with human oversight.
