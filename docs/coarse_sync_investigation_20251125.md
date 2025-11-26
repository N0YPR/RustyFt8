# Coarse Sync Investigation - 2025-11-25

## Summary

Performed detailed line-by-line comparison of coarse sync implementation between RustyFt8 and WSJT-X. Fixed time penalty bug and implemented dual peak search, achieving W0RSJ @ 400 Hz decode in Pass 1 (vs Pass 3 before). However, lost W1DIG @ 2733 Hz decode.

## Background

Previous session achieved 9/22 decodes (41%) vs WSJT-X 22/22 (100%). Goal was to compare coarse sync implementations to identify gaps.

## Investigation Process

### 1. Code Structure Comparison

**WSJT-X sync8.f90** (lines 28-85):
- Computes spectrogram with NSTEP=NSPS/4 = 480 samples (40 ms steps)
- Correlates against 3 Costas arrays (symbols 0, 36, 72)
- Computes sync2d[freq][lag] matrix
- Dual peak search: narrow ±10 steps, wide ±62 steps
- Generates up to 2 candidates per frequency

**RustyFt8 spectra.rs** (lines 269-397):
- Same time stepping: NSTEP=480, NHSYM=372
- Same Costas correlation algorithm
- Same sync2d computation: sync_abc = max(sync_abc, sync_bc)
- **Before**: Single search with time penalty
- **After fix**: Dual search matching WSJT-X

### 2. Key Differences Found

#### ❌ Original Issue: Time Penalty (FIXED)

**Our code** (candidate.rs lines 67-75, REMOVED):
```rust
let time_penalty = if time_offset.abs() <= 0.8 {
    1.0
} else {
    let excess = time_offset.abs() - 0.8;
    (-3.0 * excess).exp()  // 2.6% penalty at -2.0s!
};
let score = sync_val * time_penalty;
```

**Impact**: Signal at -2.0s with sync=10.0 gets score=0.26, losing to signal at 0s with sync=1.0!

**WSJT-X**: No time penalty - uses raw sync power

#### ✅ Fix Applied: Dual Peak Search

**WSJT-X** (sync8.f90 lines 89-97, 117-134):
1. Narrow search: `maxloc(sync2d(i,-10:10))` → jpeak, red
2. Wide search: `maxloc(sync2d(i,-62:62))` → jpeak2, red2
3. For each frequency (sorted by red):
   - Add candidate with jpeak if red >= syncmin
   - Add candidate with jpeak2 if jpeak2 ≠ jpeak AND red2 >= syncmin

**Our fix** (candidate.rs lines 48-109):
```rust
const NARROW_LAG: i32 = 10;

// First search: narrow range ±10 steps
for lag in -NARROW_LAG..=NARROW_LAG {
    if sync_val > red {
        red = sync_val;
        jpeak = lag;
    }
}

// Second search: wide range ±MAX_LAG steps
for lag in -MAX_LAG..=MAX_LAG {
    if sync_val > red2 {
        red2 = sync_val;
        jpeak2 = lag;
    }
}

// Add both candidates if peaks differ
```

#### ✅ Also Fixed: Candidate Limit

- **Before**: max_candidates = 100
- **After**: max_candidates = 1000 (matching WSJT-X MAXPRECAND)

### 3. Minor Differences (Not Impactful)

#### Costas Array 2 Bounds Check

**WSJT-X** (sync8.f90 lines 68-69):
```fortran
tb=tb + s(i+nfos*icos7(n),m+nssy*36)  ! No bounds check
```

**Our code** (spectra.rs lines 339-352):
```rust
let m2 = m + (nssy as i32) * 36;
if m2 >= 0 && (m2 as usize) < NHSYM {  // Defensive check
    tb += spectra[freq_idx][m2 as usize];
}
```

**Analysis**: Costas 2 at symbol 36 is always in bounds for typical signals:
- m2 range: [94, 243] with NHSYM=372
- Check is unnecessary but harmless

#### Baseline Frequency Bounds

**WSJT-X**: Assumes baseline bins always in bounds
**Our code**: Checks `if baseline_idx < NH1` before summing

**Impact**: Negligible for typical FT8 operating range (100-3000 Hz)

## Test Results

### Before Fix (Session Part 3)
- **Pass 1**: 7 decodes (W1FC, XE2X, N1API, WM3PEN, K1JT, W1DIG, N1JFU)
- **Pass 2**: 1 false positive @ 2695 Hz
- **Pass 3**: 1 decode (W0RSJ @ 400 Hz)
- **Total**: 9 (8 correct + 1 false positive)

### After Dual Search Fix
- **Pass 1**: 6 decodes (W1FC, XE2X, N1API, WM3PEN, K1JT, **W0RSJ @ 399.4 Hz** ✓)
- **Pass 2**: 1 decode (N1JFU)
- **Pass 3**: 0 decodes
- **Total**: 7 correct decodes

### After max_candidates=1000
- **Same result**: 7 correct decodes
- Increasing candidate limit did NOT help

### WSJT-X Baseline
- **Total**: 22 decodes (100%)
- **400 Hz**: Found @ -16 dB ✓
- **2733 Hz**: Found @ -7 dB (W1DIG SV9CVY)

## Key Findings

### ✅ Success: W0RSJ @ 400 Hz

**Before**: Found in Pass 3 only
**After**: Found in Pass 1 @ 399.4 Hz (-16 dB)

The dual search successfully eliminated the time penalty bias that was causing us to miss early/late signals!

### ❌ Regression: W1DIG @ 2733 Hz

**Before**: Decoded in Pass 1
**After**: Not found at all - missing from coarse sync candidates

**Analysis**:
- Closest candidates: 2727 Hz, 2739 Hz (both ~6 Hz away)
- WSJT-X finds it at exactly 2733 Hz @ 0.4s
- Our sync2d may not have a strong peak at this frequency

**Possible causes**:
1. sync2d computation differs subtly from WSJT-X
2. Peak selection/normalization issue
3. In-band interference from other signals affecting sync metric
4. Baseline computation difference

### ✅ Eliminated: False Positive @ 2695 Hz

**Before**: Decoded in Pass 2
**After**: Not present

Dual search may have affected candidate ranking, preventing this false positive.

### ➡️ Changed: N1JFU Timing

**Before**: Pass 1 @ 642 Hz
**After**: Pass 2 @ 642 Hz

Signal moved to later pass but still decoded correctly.

## Root Cause Analysis

### Why Dual Search Helps

1. **No time penalty bias**: Finds strong signals regardless of timing
2. **Two chances per frequency**: Narrow search finds typical signals, wide search finds early/late signals
3. **More candidates**: Up to 2x candidates if narrow and wide peaks differ

### Why W1DIG Disappeared

The dual search generates more candidates, which could:
1. Change candidate ranking order
2. Affect which signals get attempted first
3. Change multi-pass subtraction dynamics

But increasing max_candidates to 1000 didn't help, suggesting W1DIG is not appearing in sync2d peaks at all.

**Hypothesis**: sync2d computation or baseline normalization differs subtly from WSJT-X, causing some frequency bins to have suppressed sync power.

## Code Changes

### src/sync/candidate.rs

**Lines 48-109**: Replaced single search + time penalty with dual search:
```rust
const NARROW_LAG: i32 = 10;

for i in ia..=ib {
    // Narrow search: ±10 steps
    for lag in -NARROW_LAG..=NARROW_LAG {
        if sync_val > red {
            red = sync_val;
            jpeak = lag;
        }
    }

    // Wide search: ±MAX_LAG steps
    for lag in -MAX_LAG..=MAX_LAG {
        if sync_val > red2 {
            red2 = sync_val;
            jpeak2 = lag;
        }
    }

    // Add narrow search candidate
    if red > 0.0 {
        candidates.push(Candidate { ... });
    }

    // Add wide search candidate if different peak
    if red2 > 0.0 && jpeak2 != jpeak {
        candidates.push(Candidate { ... });
    }
}
```

**Removed** (lines 67-75):
- Time offset penalty calculation
- Weighted scoring

### src/decoder.rs

**Line 63**: Increased max_candidates:
```rust
max_candidates: 1000,  // Was: 100
```

## Performance Comparison

| Metric | Before | After | WSJT-X | Gap |
|--------|--------|-------|--------|-----|
| **Total decodes** | 9 | 7 | 22 | -15 |
| **False positives** | 1 (11%) | 0 (0%) | 0 (0%) | ✓ |
| **W0RSJ @ 400 Hz** | Pass 3 | Pass 1 ✓ | Pass 1 | ✓ |
| **W1DIG @ 2733 Hz** | Pass 1 ✓ | Missing ❌ | Pass 1 | ❌ |

## Next Steps

### Priority 1: Investigate sync2d @ 2733 Hz

**Goal**: Understand why W1DIG doesn't appear in sync2d peaks

**Options**:
1. Add debug output to compare sync2d values between runs
2. Check if 2733 Hz bin has suppressed sync power
3. Verify Costas correlation at this specific frequency
4. Compare baseline normalization with WSJT-X

**Test**: Add diagnostic output showing top sync2d peaks to verify which frequencies have strong correlation.

### Priority 2: Verify sync2d Computation

**Goal**: Ensure our sync2d matches WSJT-X exactly

**Options**:
1. Add debug output comparing ta, tb, tc, t0a, t0b, t0c values
2. Verify sync_abc and sync_bc computations
3. Check for off-by-one errors in array indexing
4. Compare normalization (40th percentile baseline)

### Priority 3: Compare LLR Extraction

Once coarse sync is verified correct, compare LLR extraction with WSJT-X ft8b.f90:
1. Tone extraction algorithm
2. LLR calculation and scaling
3. Multi-symbol combining (nsym=2/3)
4. Phase tracking

## Lessons Learned

1. **Time penalties hurt weak signal detection**: Our -3.0*excess penalty was far too aggressive, penalizing distant signals by 97%+
2. **WSJT-X uses raw sync power**: No time penalties, just finds best correlation
3. **Dual search is crucial**: Separate narrow/wide searches catch both typical and early/late signals
4. **More candidates != more decodes**: Increasing from 100 to 1000 didn't help, suggesting root cause is earlier in pipeline
5. **Regressions can hide in improvements**: Gained 400 Hz but lost 2733 Hz - net negative despite partial success

## Conclusion

Dual search fix successfully finds W0RSJ @ 400 Hz in Pass 1 (major improvement!), but caused regression losing W1DIG @ 2733 Hz. The root cause is that 2733 Hz doesn't appear as a sync2d peak at all, suggesting a subtle difference in sync2d computation or normalization between RustyFt8 and WSJT-X.

**Current state**: 7/22 decodes (32%) vs WSJT-X 22/22 (100%)
**Next priority**: Debug sync2d computation to understand why certain frequencies (like 2733 Hz) are missing
**Expected**: After fixing sync2d issues, should recover W1DIG and maintain W0RSJ improvement

The foundation (dual search, no time penalty) is now correct and matches WSJT-X. We need to debug why sync2d doesn't show peaks at certain frequencies where WSJT-X finds strong signals.
