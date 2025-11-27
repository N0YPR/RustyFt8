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

2. **Sync2d Computation**: 100% match with WSJT-X ✅
   - Created comprehensive test: `tests/sync/test_sync2d.rs`
   - Test validates sync2d correlation matrix (1,250 values)
   - All peak lags match exactly
   - Created Fortran reference generator: `tests/sync/test_sync2d.f90`
   - **BREAKTHROUGH**: Discovered and fixed jstrt calculation issue (see below)

3. **Candidate Selection**: **96% match rate (192/200 candidates)** ✅
   - Both implementations find 200 candidates
   - Strong and medium signals match perfectly
   - Only 8 very weak edge-case signals differ (~1.0-1.6 sync power)

### 🎯 BREAKTHROUGH: jstrt Paradox Solved!

A critical discovery resolved a fundamental inconsistency in the `jstrt` (start lag) calculation.

#### The Discovery

**The Paradox:**
- With jstrt=13 (rounded): Sync2d 100% match, but candidates 0.5% match (times 0.04s early)
- With jstrt=12 (truncated): Sync2d offset by 1, but candidates 77% match

**Root Cause Found:**
WSJT-X's sync8.f90 line 50 contains:
```fortran
jstrt=0.5/tstep
```

**NO** `nint()` call! Since `jstrt` is not explicitly declared, **Fortran's implicit typing** makes it an integer (variables starting with i-n are implicitly integers). This causes **automatic truncation**:
```
0.5 / 0.04 = 12.5 → truncates to 12 (NOT rounded to 13)
```

#### The Bug in Our Test Code

Our `test_sync2d.f90` was using:
```fortran
integer :: jstrt
jstrt = nint(0.5 / tstep)  ! WRONG: Rounds to 13
```

This gave us **incorrect reference data**! WSJT-X actually uses jstrt=12 (truncated).

#### The Fix

Updated `test_sync2d.f90` line 52 to match WSJT-X:
```fortran
jstrt = 0.5 / tstep  ! CORRECT: Truncates to 12 (matches sync8.f90)
```

Regenerated `sync2d_ref.csv` with correct jstrt=12 values.

#### Verification Results

After fixing the test reference data:
- **Sync2d**: 100% match with jstrt=12 (1250/1250 values within 0.001%)
- **Peak lags**: All match exactly
- **Candidates**: 77% match maintained (154/200)
- **Conclusion**: Our implementation is CORRECT! jstrt=12 matches WSJT-X exactly.

The "paradox" was caused by our test using wrong jstrt value, not by any bug in our implementation.

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
// CORRECT implementation (matches WSJT-X sync8.f90 line 50):
let jstrt = (0.5 / (NSTEP as f32 / SAMPLE_RATE)) as i32; // = 12 (truncated)

// NOTE: Do NOT use .round()! WSJT-X uses Fortran implicit typing which truncates.
// Using .round() would give jstrt=13, which is WRONG and breaks candidate matching.
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

### ✅ COMPLETED: jstrt Paradox Resolution
The jstrt issue is SOLVED! Our implementation is correct and matches WSJT-X exactly with jstrt=12.

### 🎉 COMPLETED: Reached 96% Candidate Match Rate!

Starting from 77% match, we identified and fixed two critical bugs:

#### Bug 1: Deduplication Kept First, Not Strongest (77% → 82%)

**Problem:** When multiple candidates were within 4 Hz and 0.04s, we kept the FIRST one encountered, even if a later candidate was stronger.

**WSJT-X behavior (sync8.f90 lines 137-144):**
```fortran
! Compare sync powers and keep the STRONGER one
if(candidate0(3,i).ge.candidate0(3,j)) candidate0(3,j)=0.
if(candidate0(3,i).lt.candidate0(3,j)) candidate0(3,i)=0.
```

**Our bug:** First-come-first-served deduplication. Example:
- Bin A narrow: sync=10.0 (processed first, added to filtered)
- Bin B wide: sync=11.0 (marked as duplicate, INCORRECTLY rejected)

**Fix:** Changed [coarse.rs:328-356](../src/sync/coarse.rs#L328-L356) to compare sync powers and replace weaker duplicates:
```rust
if cand.sync_power > existing.sync_power {
    filtered[idx] = *cand;  // Replace weaker with stronger
}
```

**Result:** 77% → 82% (10 more candidates matched)

#### Bug 2: Premature Loop Break (82% → 96%)

**Problem:** We broke the candidate generation loop after 400 candidates (`max_candidates * 2`), but WSJT-X processes up to 1000.

**Impact:** Bins ranked by weak narrow peaks but having strong wide peaks never got processed. Example:
- Bin 835 (2609.4 Hz): Narrow=1.316, Wide=**32.161** (matches WSJT-X!)
- Position in sorted order: **288** (sorted by narrow peak strength)
- We broke at ~200 bins (400 candidates), so bin 835 **never got processed**

**WSJT-X behavior (sync8.f90):**
```fortran
parameter (MAXPRECAND=1000)
do i=1,min(MAXPRECAND,iz)  ! Loop up to 1000 bins
   if(k.ge.MAXPRECAND) exit   ! Stop when 1000 candidates added
enddo
```

**Fix:** Removed premature break in [coarse.rs:268](../src/sync/coarse.rs#L268). Now process bins until hitting 1000 candidate limit (line 315), matching WSJT-X.

**Result:** 82% → 96% (20 more candidates matched, including bin 835!)

### ✅ GOAL ACHIEVED: 95% Candidate Match Rate (Currently 96%!)

**Final Status:**
- **192 out of 200 candidates match (96.0%)**
- Strong and medium signals match perfectly
- Only 8 very weak edge-case signals differ (sync 1.0-1.6)
- Remaining mismatches are in noisy 1490-1506 Hz region, likely due to numerical sensitivity

**Next Steps:**
With coarse sync now validated at 96%, the next stage in the FT8 decoding pipeline is:
1. **Fine Sync** - Refine frequency and time offsets for each candidate
2. **Symbol Extraction** - Extract 79 symbol soft-decisions using refined offsets
3. **LDPC Decoding** - Decode the 174-bit codeword to recover the 77-bit message

## Debugging Tips

### Enable Trace Logging
```bash
RUST_LOG=rustyft8::sync::spectra=trace cargo test test_coarse -- --ignored --nocapture
```

### Compare Specific Bins
The test_sync2d program outputs sync2d values for key bins (with correct jstrt=12):
- Bin 477 (1490.6 Hz) - Weak signal, peak at lag=1
- Bin 478 (1493.8 Hz) - Weak signal, peak at lag=2
- Bin 482 (1506.2 Hz) - Medium signal, peak at lag=10
- Bin 823 (2571.9 Hz) - Strong signal, peak at lag=8 (time=0.30s)
- Bin 811 (2534.4 Hz) - Strong signal, peak at lag=60 (time=2.38s)

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
