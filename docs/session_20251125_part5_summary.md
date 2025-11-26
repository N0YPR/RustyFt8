# Session Summary - 2025-11-25 Part 5: Sync2d Deep Dive

## Overview

Performed deep investigation into sync2d computation and candidate selection after implementing dual peak search. Identified that decode_top_n=50 was too low for dual search generating ~1817 candidates, causing good candidates to be filtered out.

## Starting Point

From Part 4: 7/22 decodes (32%) with dual peak search implemented
- ✅ W0RSJ @ 400 Hz found in Pass 1 (was Pass 3)
- ❌ Lost W1DIG @ 2733 Hz (was Pass 1, now missing)
- ✅ False positive @ 2695 Hz eliminated

## Investigation Process

### 1. Added sync2d Diagnostic Output

**Code changes** (candidate.rs lines 54-84):
```rust
let _debug_sync2d = true; // Enable diagnostics
for &freq in &debug_freqs {
    // Find max sync value across all lags
    // Also check narrow range ±10 separately
}
```

**Key findings for Pass 1**:
```
sync2d[400 Hz]: max=4.938 at lag=7 (0.28s), narrow_max=4.938 at lag=7
sync2d[590 Hz]: max=9.822 at lag=8 (0.32s), narrow_max=9.822 at lag=8
sync2d[2733 Hz]: max=4.446 at lag=41 (1.64s), narrow_max=2.932 at lag=3 (0.12s)
sync2d[2852 Hz]: max=3.199 at lag=29 (1.16s), narrow_max=2.243 at lag=7
```

**Observation**: 2733 Hz HAS sync2d peaks but at wrong time offsets:
- WSJT-X finds W1DIG @ 2733 Hz at +0.4s (lag ~10)
- Our narrow_max is at lag=3 (0.12s) with sync=2.932
- Our wide_max is at lag=41 (1.64s) with sync=4.446

### 2. Checked Normalization

**Baseline**: 40th percentile of 1817 candidates = 1.944

**2733 Hz normalization**:
```
sync=2.932 → normalized=1.508 ✓ (above sync_min=0.5)
sync=4.446 → normalized=2.287 ✓ (above sync_min=0.5)
```

Both candidates pass normalization! Not the problem.

### 3. Checked Deduplication Filter

**Tracked 2733 Hz through filtering**:
```
2733 Hz candidate PASSED: sync=1.508, time=0.10s ✓
2733 Hz candidate PASSED: sync=2.287, time=1.62s ✓
2733 Hz candidate PASSED: sync=1.673, time=0.38s ✓
```

Candidates pass deduplication! Not the problem.

### 4. Discovered Root Cause: decode_top_n Too Low

**Code** (decoder.rs line 117):
```rust
let decode_results: Vec<DecodeResult> = candidates
    .iter()
    .take(config.decode_top_n) // Only process top 50!
    .enumerate()
```

**Problem**: With 1817 candidates and decode_top_n=50, only the top 50 by sync power are processed.

**2733 Hz candidates**:
- normalized=1.508 (narrow)
- normalized=2.287 (wide)
- normalized=1.673 (nearby bin)

With many stronger candidates (e.g., 2157 Hz: 5.681, 590 Hz: 5.052), these get ranked below 50th place!

## Fix Applied

### Increased decode_top_n from 50 to 100

**Rationale**: Dual search generates ~2x candidates per frequency (if narrow/wide peaks differ), so need higher limit.

**Code change** (decoder.rs line 64):
```rust
decode_top_n: 100, // Was: 50. Dual search generates ~2x candidates, need higher limit
```

## Results

### Before Fix (decode_top_n=50)
- **Pass 1**: 6 decodes
- **Pass 2**: 1 decode
- **Pass 3**: 0 decodes
- **Total**: 7/22 (32%)

### After Fix (decode_top_n=100)
- **Pass 1**: 8 decodes
- **Pass 2**: 0 decodes
- **Total**: 8/22 (36%)

**Improvements**:
- ✅ **K1JT EA3AGB @ 1649 Hz** - NEW decode!
- ✅ **N1JFU @ 642 Hz** - moved from Pass 2 to Pass 1

**Status of W1DIG @ 2733 Hz**:
- ❌ Still not decoded
- ✅ Now reaches fine_sync: `FINE_SYNC: freq=2730.5 Hz, dt_in=1.62s`
- ✅ Gets refined: `REFINED: freq_in=2730.5 -> freq_out=2728.5 Hz, dt_out=1.68s, sync_out=0.115`
- ✅ Gets extracted: `EXTRACT: freq=2728.5 Hz, dt=1.68s, nsym=1`
- ❌ LDPC decoding fails (no output)

### WSJT-X Baseline
- **Total**: 22/22 (100%)
- **W1DIG**: Found @ 2733 Hz, +0.4s, -7 dB

## Root Cause Analysis

### Why 2733 Hz Now Processed But Doesn't Decode

**Timing discrepancy**:
- WSJT-X: 2733 Hz @ +0.4s (lag ~10)
- Our sync2d: No strong peak at lag ~10
  - narrow_max at lag=3 (0.12s): sync=2.932
  - wide_max at lag=41 (1.64s): sync=4.446

**Our candidate**: Extracted at 2728.5 Hz, 1.68s → 1.28 seconds off from correct time!

**Hypothesis**: W1DIG signal has sync2d peak near lag=10 (0.4s), but it's weaker than peaks at lag=3 and lag=41, so our dual search selects wrong peaks.

### Why decode_top_n=50 Was Insufficient

**Dual search impact**:
- Frequency range: 100-3000 Hz = ~930 bins
- Each bin can generate 1-2 candidates (if narrow/wide peaks differ)
- Total candidates: 1817 (confirming ~2x multiplier)

**Ranking**:
- Strong signals like 2157 Hz (normalized=5.681) rank high
- Moderate signals like 2733 Hz (normalized=1.508-2.287) rank lower
- With only top 50 processed, many good candidates lost

## Comparison with WSJT-X Approach

### WSJT-X Strategy

Iterates through **frequencies** sorted by sync power (sync8.f90 lines 117-134):
```fortran
do i=1,min(MAXPRECAND,iz)
   n=ia + indx(iz+1-i) - 1  ! Get frequency bin in sorted order
   if (red(n).ge.syncmin) then
      ! Add narrow search candidate
   endif
   if (jpeak2(n) != jpeak(n)) then
      ! Add wide search candidate if different peak
   endif
enddo
```

**Key insight**: WSJT-X processes frequencies in order, adding both narrow/wide candidates for each frequency before moving to next frequency.

### Our Strategy

Generates all candidates, sorts globally, takes top N:
```rust
candidates.iter().take(config.decode_top_n)
```

**Problem**: Lower-ranked candidates from same frequency bin get cutoff, even if the frequency bin itself is strong.

### Better Approach (Future Work)

Match WSJT-X more closely:
1. Sort frequency bins by their best sync power
2. For each frequency (in order), add narrow + wide candidates if they meet syncmin
3. Stop after processing N frequency bins or max_candidates reached

This ensures each frequency bin gets fair representation before cutoff.

## Performance Comparison

| Metric | Part 4 | Part 5 | WSJT-X | Gap |
|--------|--------|--------|--------|-----|
| **Total decodes** | 7 (32%) | 8 (36%) | 22 (100%) | -64% |
| **W0RSJ @ 400 Hz** | Pass 1 ✓ | Pass 1 ✓ | Pass 1 ✓ | Match |
| **K1JT EA3AGB @ 1649 Hz** | Missing | Pass 1 ✓ | Pass 1 ✓ | Match |
| **W1DIG @ 2733 Hz** | Missing | Attempted | Pass 1 ✓ | Wrong time |
| **decode_top_n** | 50 | 100 | ~1000 | -900 |
| **Candidates generated** | 1817 | 1817 | ~1000 | +817 |

## Code Changes

### src/decoder.rs

**Line 64**: Increased decode_top_n:
```rust
decode_top_n: 100, // Was: 50
```

### src/sync/candidate.rs

**Lines 54-84**: Added sync2d diagnostic output (disabled by default):
```rust
let _debug_sync2d = false; // Enable to see sync2d values
if _debug_sync2d {
    for &freq in &debug_freqs {
        let max_sync = sync2d[bin].iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
            .map(|(idx, val)| (idx as i32 - MAX_LAG, *val))
            .unwrap_or((0, 0.0));

        let narrow_max = (-NARROW_LAG..=NARROW_LAG).map(|lag| {
            let idx = (lag + MAX_LAG) as usize;
            if idx < sync2d[bin].len() {
                (lag, sync2d[bin][idx])
            } else {
                (lag, 0.0)
            }
        }).max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
        .unwrap_or((0, 0.0));

        eprintln!("sync2d[{:.0} Hz (bin {})]: max={:.3} at lag={} ({:.2}s), narrow_max={:.3} at lag={} ({:.2}s)",
            freq, bin, max_sync.1, max_sync.0, max_sync.0 as f32 * tstep,
            narrow_max.1, narrow_max.0, narrow_max.0 as f32 * tstep);
    }
}
```

**Lines 151-164**: Added normalization diagnostic output (disabled by default):
```rust
if _debug_sync2d {
    eprintln!("Normalization: {} candidates, 40th percentile baseline = {:.3}",
        candidates.len(), baseline);
    for &bin in &debug_bins {
        let freq = bin as f32 * df;
        for cand in candidates.iter().filter(|c| ((c.frequency / df) as usize) == bin) {
            let normalized = cand.sync_power / baseline;
            eprintln!("  {:.0} Hz: sync={:.3} → normalized={:.3}",
                freq, cand.sync_power, normalized);
        }
    }
}
```

**Lines 188-201**: Added deduplication tracking (disabled by default):
```rust
let is_2733 = (cand.frequency - 2733.0).abs() < 5.0;
if _debug_sync2d && is_2733 {
    if is_dupe {
        eprintln!("  2733 Hz candidate FILTERED as dupe of {:.0} Hz: sync={:.3}, time={:.2}s",
            dupe_of.unwrap(), cand.sync_power, cand.time_offset);
    } else if cand.sync_power < sync_min {
        eprintln!("  2733 Hz candidate FILTERED (sync < {:.1}): sync={:.3}, time={:.2}s",
            sync_min, cand.sync_power, cand.time_offset);
    } else {
        eprintln!("  2733 Hz candidate PASSED: sync={:.3}, time={:.2}s",
            cand.sync_power, cand.time_offset);
    }
}
```

## Documentation Created

- [docs/session_20251125_part5_summary.md](session_20251125_part5_summary.md): This document
- Updated [NEXT_STEPS.md](../NEXT_STEPS.md) with current status

## Next Steps

### Priority 1: Investigate Timing Discrepancy for 2733 Hz

**Goal**: Understand why we don't see a strong sync2d peak at lag ~10 (0.4s) for 2733 Hz

**Options**:
1. Add detailed sync2d profile showing all lag values for 2733 Hz bin
2. Compare Costas correlation at correct time vs. our peak times
3. Check if in-band interference affects sync2d at specific lags
4. Verify WSJT-X sync2d computation matches ours exactly

**Expected**: Find subtle difference in sync2d computation or understand why correct peak is weak

### Priority 2: Consider Per-Frequency Candidate Selection

**Goal**: Match WSJT-X's frequency-ordered approach instead of global candidate sort

**Implementation**:
1. Group candidates by frequency bin
2. Sort frequency bins by their best candidate's sync power
3. For each bin (in order), add narrow + wide candidates
4. Stop after N bins or max_candidates reached

**Expected**: Better representation of all frequency bins, especially moderate-strength signals

### Priority 3: Compare LLR Extraction with WSJT-X

Once sync issues resolved, investigate why LDPC fails for candidates that reach extraction:
1. Compare LLR values with WSJT-X ft8b.f90
2. Verify tone extraction algorithm
3. Check LLR scaling and normalization

## Lessons Learned

1. **Dual search doubles candidates**: decode_top_n must account for ~2x candidate generation
2. **Global sorting can be unfair**: Lower-ranked candidates from strong frequency bins get lost
3. **Diagnostics are essential**: sync2d, normalization, and filtering diagnostics revealed the pipeline
4. **Timing is critical**: W1DIG candidate at wrong time (1.68s vs 0.4s) causes decode failure
5. **Progressive narrowing**: Good candidates can survive coarse sync, normalization, dedup but still fail due to decode_top_n limit

## Conclusion

**Root cause found**: decode_top_n=50 too low for dual search generating 1817 candidates. Increasing to 100 recovered 1 additional signal (K1JT EA3AGB @ 1649 Hz).

**Current state**: 8/22 decodes (36%) vs WSJT-X 22/22 (100%)

**W1DIG status**: Now reaches fine_sync and extraction but at wrong time offset (1.68s vs 0.4s), causing LDPC to fail. The sync2d peak at the correct time (~0.4s) is weaker than peaks at other times, suggesting:
- Sync2d computation differs subtly from WSJT-X
- In-band interference affects correlation at specific times
- Costas array correlation has timing bias

**Next priority**: Debug why sync2d doesn't show strong peak at correct time for 2733 Hz, potentially requiring line-by-line comparison of sync2d computation with WSJT-X.

The dual peak search is working correctly - we just need to understand why some signals' sync2d peaks appear at wrong times.
