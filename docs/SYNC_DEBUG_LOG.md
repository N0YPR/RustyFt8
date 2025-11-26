# Sync8 Debugging Log

## Session 2025-11-26: Off-by-One Array Indexing Bug

### Problem
Test `test_coarse_sync_matches_wsjtx` was failing with only 4.5% match rate (9/200 candidates).

### Root Causes Found

#### 1. Fortran 1-indexed vs Rust 0-indexed Arrays
**BUG:** In `src/sync/spectra.rs`, we were using `m` directly as a Rust array index when it should be `m-1`.

WSJT-X uses Fortran's 1-indexed arrays:
```fortran
if(m.ge.1.and.m.le.NHSYM) then
    ta=ta + s(i+nfos*icos7(n),m)  ! m=1 accesses first element
```

RustyFt8 was incorrectly using:
```rust
if m >= 1 && (m as usize) < NHSYM {
    ta += spectra[freq_idx][m as usize];  // m=1 accesses SECOND element!
}
```

**FIX:** Convert Fortran 1-indexed to Rust 0-indexed:
```rust
if m >= 1 && m <= NHSYM as i32 {
    let time_idx = (m - 1) as usize;  // m=1 → index 0
    ta += spectra[freq_idx][time_idx];
}
```

**RESULT:** Match rate improved from 2.5% → 38.5% (77/200 candidates)

#### 2. Percentile Rounding
**MINOR FIX:** WSJT-X uses `nint(0.40*iz)` which rounds to nearest integer, but we were truncating.

Changed from:
```rust
let percentile_idx = (nbins as f32 * 0.4) as usize;  // truncates
```

To:
```rust
let percentile_idx = (nbins as f32 * 0.4).round() as usize;  // rounds
```

**RESULT:** No significant impact on match rate (still 38.5%)

### Remaining Issues

#### 1. Sync Power Still Off by ~7x
- 2571.9 Hz: WSJT-X=237.840, RustyFt8=35.780 (6.6x difference)
- 2534.4 Hz: WSJT-X=67.523, RustyFt8=34.831 (1.9x difference)
- Baseline calculation: 1.683 (seems reasonable, values around it are 1.681-1.688)

#### 2. **CRITICAL: Sync2d Peaks at Wrong Time Lags**

Trace logging reveals sync2d correlation peaks are at completely wrong lags:

| Frequency | Expected jpeak | Expected time | Found jpeak | Found time | Error |
|-----------|----------------|---------------|-------------|------------|-------|
| 1490.6 Hz | 1-2            | 0.020s        | 3           | 0.100s     | +2 steps (0.08s) |
| 1493.8 Hz | 2-3            | 0.060s        | 1           | 0.020s     | -1 step (0.04s) |
| 1506.2 Hz | 10             | 0.380s        | 0           | -0.020s    | -10 steps (0.4s) |
| 2571.9 Hz | 8-9            | 0.300s        | 2           | 0.060s     | -6 steps (0.24s) |
| 2534.4 Hz | 60             | 2.380s        | 3           | 0.100s     | -57 steps (2.28s) |

**Formula:** `time_offset = (jpeak - 0.5) * tstep` where `tstep = 0.04s`

**Analysis:**
- Large systematic offset in time peaks (not random errors)
- Neither narrow (±10) nor wide (±62) search finding correct peaks
- Wide search sometimes finds negative peaks (-62, -58, -12) which are clearly wrong
- This indicates the sync2d correlation computation itself has issues

**Hypothesis:** There may be another array indexing bug, or the time alignment calculation in compute_sync2d() is incorrect.

### Next Steps

1. **Investigate sync2d computation** in `src/sync/spectra.rs`:
   - Check if there are more Fortran/Rust array indexing mismatches
   - Verify `jstrt` calculation
   - Verify time index calculation for all three Costas arrays
   - Check if `m`, `m2`, `m3` calculations are correct

2. **Add detailed sync2d tracing:**
   - Log sync2d[823][-10..10] values to see actual correlation peak shape
   - Compare with WSJT-X sync2d output at same bins
   - Identify where the peak should be vs where it actually is

3. **Check if spectra computation is correct:**
   - Verify spectra power values are reasonable
   - Check if spectra bins align with WSJT-X expectations

### Test Command

```bash
# Run with trace logging to see sync2d peak details
RUST_LOG=rustyft8::sync::coarse=trace cargo test test_coarse_sync_matches_wsjtx -- --ignored --nocapture
```

### References

- WSJT-X sync8.f90: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/sync8.f90`
- Test reference data: Generated from WSJT-X sync8() output on `tests/test_data/210703_133430.wav`
- Fortran arrays are 1-indexed, Rust arrays are 0-indexed - **ALWAYS SUBTRACT 1** when converting!
