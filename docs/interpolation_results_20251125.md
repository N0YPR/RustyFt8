# Parabolic Interpolation Results - 2025-11-25

## Summary

Implemented parabolic interpolation in both coarse sync and fine sync, but **still 8/22 decodes**. The interpolation works correctly, but reveals a deeper problem: **sync2d peaks at wrong frequency bins**.

## Implementation

### Fine Sync Interpolation ✅ WORKING

**Location**: [src/sync/fine.rs](../src/sync/fine.rs#L215-L244)

**Algorithm**:
1. Discrete search: ±2.5 Hz in 0.5 Hz steps (11 test frequencies)
2. Find peak: best_freq with best_sync
3. Interpolate: Fit parabola through [best_freq-0.5, best_freq, best_freq+0.5]
4. Refine: `best_freq += delta * 0.5` where `delta = 0.5 * (s2 - s0) / denom`

**Results**: Small adjustments (0.03-0.12 Hz), working correctly

**Example (F5RXL)**:
- Discrete peak: 1196.8125 Hz
- Interpolated: 1196.7822 Hz (Δ=-0.030 Hz)
- Still 0.22 Hz off from 1197 Hz (WSJT-X)

### Coarse Sync Interpolation ✅ WORKING (but revealing underlying issue)

**Location**: [src/sync/candidate.rs](../src/sync/candidate.rs#L122-L166)

**Algorithm**:
1. Find peak bin `i` with highest sync power
2. Get sync values at bins i-1, i, i+1 (at same time lag)
3. Check if clean peak: `s1 > s0 && s1 > s2`
4. Interpolate: `freq = (i + delta) * df` where `delta = 0.5 * (s2 - s0) / denom`

**Results**: Interpolation working, but **sync2d peaks at wrong bins!**

**F5RXL frequency bins** (df=2.93 Hz/bin):
- Bin 407 (1192.4 Hz): sync=**14.635** ← HIGHEST (but 4.6 Hz off!)
- Bin 408 (1195.3 Hz): sync=4.247 (not a peak, s0 > s1)
- Bin 409 (1198.4 Hz): sync=1.841 (closest to 1197 Hz, but LOW sync!)
- Bin 410 (1201.3 Hz): sync=1.902

**Result**:
- Coarse sync picks bin 407 (highest sync=14.635)
- Creates candidate at 1192.3 Hz (after slight interpolation)
- Fine sync searches [1189.8 ... 1194.8] Hz
- Can't reach 1197 Hz!

## Root Cause: Sync2D Distribution Wrong

The interpolation revealed that **sync2d itself has peaks at the wrong frequencies**. For F5RXL @ 1197 Hz:

**Expected** (for WSJT-X to succeed):
- Bin 409 (1198.4 Hz) should have highest sync power
- Would enable fine sync to find 1197 Hz

**Actual** (what we see):
- Bin 407 (1192.4 Hz) has highest sync power
- Bins 408-409 have much lower sync
- 4.6 Hz error in peak location!

This 4.6 Hz error (>1.5 frequency bins) suggests:
1. Costas array correlation has frequency bias
2. Signal processing (FFT windowing, binning) differs from WSJT-X
3. Baseline normalization shifts peak locations
4. Or combination of these factors

## Why Interpolation Didn't Help

**Fine sync interpolation**: Refines by 0.03-0.12 Hz, but starting from wrong coarse frequency (1195.3 Hz instead of ~1198 Hz), can't reach 1197 Hz.

**Coarse sync interpolation**: Works correctly, but interpolating between wrong bins (407-408) doesn't fix the underlying problem that bin 409 has low sync power.

**Analogy**: Interpolation is like measuring distance very precisely (0.01 Hz accuracy), but we're measuring the wrong peak! The real signal is elsewhere.

## Next Steps

### Priority 1: Investigate sync2d Peak Shift ⚠️ CRITICAL

**Goal**: Understand why sync2d peaks 4.6 Hz away from actual signal

**Investigation**:
1. **Compare Costas correlation** with WSJT-X sync8.f90 lines 56-83
2. **Check FFT parameters**: window functions, binning, scaling
3. **Baseline normalization**: Does 40th percentile shift affect peak locations?
4. **Spectral analysis**: Plot sync2d[407:410] across all time lags
5. **Signal processing**: Any frequency-dependent bias in our pipeline?

**Expected outcome**: Identify why sync power peaks at bin 407 instead of bin 409

### Priority 2: Test Other Missing Signals

Check if the same pattern (sync2d peaks ~5 Hz off) affects other missing signals:
- K1BZM EA3GP @ 2695 Hz
- DL8YHR @ 2609 Hz
- N1PJT HB9CQK @ 466 Hz

**If yes**: Systematic frequency bias in sync2d computation
**If no**: F5RXL has unique issue (interference, etc.)

### Priority 3: Match WSJT-X Sync8 Line-by-Line

**Goal**: Eliminate any algorithmic differences in sync2d computation

**Tasks**:
1. Compare spectra.rs FFT with WSJT-X four2a
2. Verify Costas pattern application (COSTAS_PATTERN vs icos7)
3. Check sync power accumulation formula
4. Match baseline computation exactly
5. Verify time lag indexing

## Alternative: Increase Coarse Search Range

If sync2d bias is hard to fix, could work around it:

**Option**: Fine sync search ±5 Hz (was ±2.5 Hz)
- Would allow 1192.3 Hz candidate to reach 1197.3 Hz
- Cost: 2x more downsample operations
- Benefit: Robust to coarse sync errors up to 5 Hz

## Status

- ✅ Parabolic interpolation implemented (coarse + fine)
- ✅ Interpolation working correctly (0.03-0.12 Hz refinements)
- ❌ **Still 8/22 decodes** - no improvement
- ⚠️ **Root cause identified**: sync2d peaks at wrong bins (4.6 Hz off for F5RXL)

## Files Modified

- [src/sync/fine.rs](../src/sync/fine.rs): Added parabolic interpolation after discrete frequency search
- [src/sync/candidate.rs](../src/sync/candidate.rs): Added parabolic interpolation for coarse sync bins

## References

- [sub_bin_accuracy_investigation.md](sub_bin_accuracy_investigation.md): Investigation of WSJT-X sub-bin methods
- [session_20251125_part8_sync_fix_results.md](session_20251125_part8_sync_fix_results.md): Sync score preservation fix
- [tone_extraction_root_cause.md](tone_extraction_root_cause.md): Why 0.2 Hz error causes 20% tone errors
- WSJT-X sync8.f90 lines 56-83: Costas array sync correlation
