# Sync2D Bounds Check Fix Results - 2025-11-25

## Summary

Fixed algorithm differences in `compute_sync2d()` to match WSJT-X, removing frequency bounds checks on Costas tone and baseline accumulation. **Result: Sync scores dramatically changed but still 8/22 decodes.**

---

## Changes Made

### src/sync/spectra.rs (lines 319-370)

**Before**: Multiple frequency bounds checks
**After**: Match WSJT-X exactly - no frequency bounds checks

#### Change 1: Removed Frequency Check on Costas Tone

**Before**:
```rust
if m >= 0 && (m as usize) < NHSYM {
    let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
    if freq_idx < NH1 {  // ← Extra frequency check!
        ta += spectra[freq_idx][m as usize];
        // baseline inside freq check!
        for k in 0..7 {
            let baseline_idx = i + nfos * k;
            if baseline_idx < NH1 {  // ← Another frequency check!
                t0a += spectra[baseline_idx][m as usize];
            }
        }
    }
}
```

**After** (matching WSJT-X sync8.f90:64-67):
```rust
if m >= 1 && (m as usize) < NHSYM {
    let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
    // NO frequency check - trust that ia/ib set correctly
    ta += spectra[freq_idx][m as usize];

    // Baseline ALWAYS computed when time is in bounds
    for k in 0..7 {
        let baseline_idx = i + nfos * k;
        t0a += spectra[baseline_idx][m as usize];  // NO frequency check!
    }
}
```

#### Change 2: Keep Minimal Bounds Check for Safety

Middle and third Costas still have time bounds checks to prevent Rust panics, but frequency checks removed.

---

## Impact on Sync Scores

### Before Fix (With Frequency Bounds Checks)

| Signal | Frequency | Sync Score | Notes |
|--------|-----------|------------|-------|
| W1FC F5BZB | 2572.7 Hz | **50.69** | Huge! |
| XE2X HA2NP | 2854.9 Hz | **0.10** | Tiny! |
| N1API HA6FQ | 2238.3 Hz | 0.32 | |
| WM3PEN EA6VQ | 2157.2 Hz | **111.78** | Massive! |
| K1JT HA0DU | 589.4 Hz | 0.20 | |
| W0RSJ EA3BMU | 399.4 Hz | 0.07 | Tiny! |
| N1JFU EA6EE | 642.0 Hz | 0.11 | |
| K1JT EA3AGB | 1649.8 Hz | 0.07 | Tiny! |

**Range**: 0.07 to 111.78 (1,597x variation!)

### After Fix (Without Frequency Bounds Checks)

| Signal | Frequency | Sync Score | Change |
|--------|-----------|------------|--------|
| W1FC F5BZB | 2572.7 Hz | **7.56** | ÷6.7 |
| XE2X HA2NP | 2854.5 Hz | **6.20** | ×62 |
| N1API HA6FQ | 2238.1 Hz | 5.96 | ×18 |
| WM3PEN EA6VQ | 2157.2 Hz | **5.68** | ÷20 |
| K1JT HA0DU | 589.3 Hz | 5.05 | ×25 |
| W0RSJ EA3BMU | 399.1 Hz | 4.71 | ×67 |
| N1JFU EA6EE | 642.0 Hz | 3.25 | ×30 |
| K1JT EA3AGB | 1649.8 Hz | 2.72 | ×39 |

**Range**: 2.72 to 7.56 (2.8x variation) ✅ Much more normalized!

---

## Analysis

### What Changed ✅

**Sync score normalization**: The fix dramatically normalized sync scores, reducing the range from 1,597x to 2.8x. This proves we:
1. Successfully matched WSJT-X's baseline computation algorithm
2. Fixed the conditional baseline accumulation issue
3. Removed frequency-dependent biases in sync metric

**Decode count unchanged**: Still 8/22 messages decoded (same 8 as before)

### Why Still 8/22 Decodes? ❓

Several possibilities:

#### Hypothesis 1: Frequency Peak Still at Wrong Bin
The normalization might have changed sync scores globally, but the **relative** peak locations might still be wrong. For example:
- Before: Bin 407 sync=100, Bin 409 sync=10 → peak at 407
- After: Bin 407 sync=5.0, Bin 409 sync=3.0 → **still** peak at 407!

Need to check if F5RXL is now found at bin 409 (~1198 Hz) instead of bin 407 (~1192 Hz).

#### Hypothesis 2: Another Algorithm Difference
Possible remaining differences:
1. **FFT implementation**: Our `fft_real()` vs WSJT-X's `four2a()`
2. **Spectral baseline**: Our 40th percentile vs WSJT-X's exact formula
3. **Frequency mapping**: Off-by-one errors in `i+nfos*tone` calculation
4. **Phase/time handling**: Subtle differences in time index calculations

#### Hypothesis 3: Fine Sync Still Inaccurate
Even if coarse sync finds correct bins, fine sync might still have 0.2-0.5 Hz errors (parabolic interpolation reduces but doesn't eliminate quantization).

#### Hypothesis 4: Bounds Check on Middle Costas
We kept `if m2 >= 0 && (m2 as usize) < NHSYM` for safety, but WSJT-X has NO check. For signals with large negative time offsets, this could cause different tb/t0b values, affecting sync_bc vs sync_abc selection.

---

## Next Steps

### Priority 1: Check F5RXL Frequency ⚠️ URGENT

**Goal**: Verify if sync2d now peaks at correct bin

**Test**:
```bash
cargo test test_real_ft8_recording_210703_133430 -- --ignored --nocapture 2>&1 | grep -E "(F5RXL|1197|1192|1196)"
```

**Expected**:
- Coarse sync should find F5RXL at ~1197-1198 Hz (bin 409), not 1192-1195 Hz (bin 407-408)
- If yes: Problem is in fine sync or extraction
- If no: Need to investigate other sync2d differences

### Priority 2: Add Debug Output to Sync2D

**Goal**: Understand sync2d distribution around F5RXL

**Add to spectra.rs**:
```rust
// After computing sync2d, for frequency ~1197 Hz:
if i >= 405 && i <= 412 {
    eprintln!("Bin {} ({:.1} Hz): sync[j=0] = {:.3}",
              i, i as f32 * df, sync2d[i][MAX_LAG as usize]);
}
```

### Priority 3: Remove Middle Costas Bounds Check

**Goal**: Match WSJT-X exactly (they have NO bounds check)

**Risk**: Could panic if m2 out of bounds, but WSJT-X assumes it never happens for 15s recordings

**Change**:
```rust
// Remove this check:
// if m2 >= 0 && (m2 as usize) < NHSYM {

// Always compute middle Costas (matching WSJT-X):
let m2 = (m + (nssy as i32) * 36) as usize;
tb += spectra[freq_idx2][m2];
```

### Priority 4: Compare FFT Implementations

**Goal**: Ensure our FFT matches WSJT-X's four2a

**Check**:
1. Window functions (we use no window, WSJT-X uses no window ✓)
2. FFT scaling factors
3. Real-to-complex FFT packing
4. Phase conventions

---

## Code Reference

- **Modified file**: [src/sync/spectra.rs](../src/sync/spectra.rs#L319-L370)
- **WSJT-X reference**: wsjtx/lib/ft8/sync8.f90 lines 62-74
- **Analysis doc**: [sync2d_algorithm_differences.md](sync2d_algorithm_differences.md)
- **Previous findings**: [interpolation_results_20251125.md](interpolation_results_20251125.md)

---

## Conclusion

The fix successfully **normalized sync scores** (2.8x range vs 1597x before), proving we matched WSJT-X's baseline computation algorithm. However, **decode count unchanged at 8/22**, suggesting:

1. ✅ Baseline algorithm now matches WSJT-X
2. ❌ Sync2d peak locations might still be wrong (need to verify)
3. ❌ Or there's another algorithm difference we haven't found

**Next step**: Check if F5RXL coarse sync now finds ~1197 Hz instead of ~1192 Hz.
