# Implementation Differences: RustyFt8 vs WSJT-X

Analysis of key differences between RustyFt8 and WSJT-X (`ft8b.f90`) that may explain the hard error gap on weak signals.

---

## TODO List (Priority Order)

### 1. [x] Re-Downsample After Frequency Correction ⭐⭐⭐ TESTED

**Priority:** HIGH → **CLOSED** (tested, no improvement)
**Files:** [extract.rs](../src/sync/extract.rs)
**Status:** Characterized - phase rotation is sufficient

#### Problem (Original Hypothesis)

After finding the best frequency offset, WSJT-X re-downsamples the entire signal at the corrected frequency. RustyFt8 only applies phase correction to the already-downsampled buffer.

#### WSJT-X Reference (`ft8b.f90:140-141`)

```fortran
call ft8_downsample(dd0,.false.,f1,cd0)   !Mix f1 to baseband and downsample
```

After finding `delfbest`, WSJT-X updates `f1 = f1 + delfbest` and re-downsamples.

#### Characterization Results (December 2024)

Multiple re-downsample implementations were tested against the 210703_133430.wav reference recording:

| Approach | Hard Error Gap | Decodes | K1JT HA5WA 73 | False Positives |
|----------|---------------|---------|---------------|-----------------|
| **Baseline (phase rotation)** | **12.4** | **21** | ✓ Decoded | 0 |
| Re-downsample (pure) | 12.7 | 20 | ✗ Missing | 1 |
| Re-downsample (hybrid threshold) | 12.7 | 20-21 | ✗ Missing | 1 |
| Re-downsample (≥1.0 Hz only) | 12.7 | 20 | ✗ Missing | 1 |

**Consistent false positive:** "II0XXJ 614BHK EJ78" appeared with ALL re-downsample variants.

#### Why Re-Downsample Failed

1. **FFT spectral leakage**: Re-downsampling uses FFT extraction which introduces subtle spectral artifacts
2. **Filter edge effects**: The 101-sample taper creates different edge characteristics than phase rotation
3. **Circular shift precision**: Integer bin shifting (`cshift`) is less precise than continuous phase rotation

For small corrections (±1.0 Hz), phase rotation is mathematically equivalent but avoids FFT artifacts.

#### Conclusion

**Phase rotation is sufficient** for corrections within ±1.0 Hz range. The original hypothesis that re-downsampling would improve weak signal decoding was incorrect. The hard error gap on weak signals is not caused by phase rotation error accumulation.

**Next investigation**: Items 3-4 (timing/signal flow) or LLR calculation differences may be the actual cause of the gap.

---

### 2. [x] Use Per-Symbol Frequency Tweak Instead of Phase Rotation ⭐⭐⭐ TESTED

**Priority:** HIGH → **CLOSED** (tested, no improvement)
**Files:** [extract.rs](../src/sync/extract.rs)
**Status:** Implemented - mathematically equivalent to whole-buffer approach

#### Problem (Original Hypothesis)

WSJT-X uses `sync8d` with a complex tweak vector (`ctwk`) that applies frequency correction per-symbol. RustyFt8 applies phase correction to the entire buffer, accumulating errors.

#### WSJT-X Reference (`ft8b.f90:119-133`)

```fortran
do ifr=-5,5                              !Search over +/- 2.5 Hz
  delf=ifr*0.5
  dphi=twopi*delf*dt2
  phi=0.0
  do i=1,32
    ctwk(i)=cmplx(cos(phi),sin(phi))
    phi=mod(phi+dphi,twopi)
  enddo
  call sync8d(cd0,ibest,ctwk,1,sync)     ! ← Uses sync8d with frequency tweak
enddo
```

#### Implementation (December 2024)

Implemented `build_ctwk()` function in extract.rs to create per-symbol frequency tweak vectors matching WSJT-X. Updated frequency search to use ctwk during sync measurement:

```rust
// Build per-symbol frequency tweak vector (like WSJT-X ft8b.f90:122-127)
let ctwk = build_ctwk(freq_correction, actual_sample_rate);

// Test sync quality with per-symbol tweak applied during correlation
let sync = sync_downsampled(&cd, time_offset_samples, Some(&ctwk), true, Some(actual_sample_rate));
```

#### Characterization Results (December 2024)

| Approach | Hard Error Gap | Decodes | False Positives |
|----------|---------------|---------|-----------------|
| Whole-buffer phase correction | 12.4 | 21 | 0 |
| **Per-symbol ctwk (WSJT-X style)** | **12.4** | **21** | **0** |

**No change in results** - both approaches produce identical sync measurements.

#### Why No Improvement

The per-symbol and whole-buffer approaches are **mathematically equivalent** for sync measurement:
- Both apply the same frequency shift to the correlation
- Per-symbol applies tweak to reference waveform: `wave' = wave × ctwk`
- Whole-buffer applies conjugate to signal: `cd' = cd × exp(jφ)`
- Correlation: `cd × conj(wave × ctwk) ≡ (cd × exp(jφ)) × conj(wave)`

The per-symbol approach is cleaner code but doesn't change the underlying math.

#### Conclusion

**The hard error gap is not caused by phase accumulation during frequency search.** The gap must originate from a different stage in the pipeline (likely LLR calculation or symbol extraction).

---

### 3. [ ] Remove Redundant Timing Search in `extract_symbols` ⭐⭐ IMPORTANT

**Priority:** MEDIUM
**Files:** [extract.rs](../src/decode/extract.rs), [fine.rs](../src/sync/fine.rs)
**Impact:** Prevents timing drift from optimal position

#### Problem

`extract_symbols` does an additional ±10 sample timing search after `fine_sync` has already found the optimal timing.

#### WSJT-X Flow (`ft8b.f90`)

1. Initial timing search ±10 samples (lines 110-116)
2. Frequency peaking ±2.5 Hz (lines 119-133)
3. Re-downsample at corrected frequency (line 140)
4. Final timing search ±4 samples (lines 143-152)

#### RustyFt8 Flow

1. Initial timing search ±10 samples in `fine_sync` ✓
2. Frequency search ±4 Hz with re-downsample ✓
3. Final timing search ±4 samples in `fine_sync` ✓
4. **`extract_symbols` does another ±10 sample search** ← Redundant

#### Fix

Remove the redundant timing search in `extract_symbols` (line 213) or ensure it uses a narrower range.

---

### 4. [ ] Pass Downsampled Signal from `fine_sync` to `extract_symbols` ⭐⭐ IMPORTANT

**Priority:** MEDIUM
**Files:** [extract.rs](../src/decode/extract.rs), [fine.rs](../src/sync/fine.rs)
**Impact:** Preserves frequency correction work

#### Problem

`extract_symbols` re-downsamples the signal at `candidate.frequency`, throwing away the frequency correction found in `fine_sync`.

#### Root Cause Analysis

This explains the strong correlation between `nsync` and hard error gap:

| Scenario | What Happens | Result |
|----------|--------------|--------|
| High nsync (≥18) | Initial frequency was already close | Re-downsampling at `candidate.frequency` doesn't hurt much → small gap |
| Low nsync (≤11) | `fine_sync` found a better frequency | `extract_symbols` discards it → large gap |

#### Fix

Pass the downsampled signal and timing from `fine_sync` to `extract_symbols` rather than having `extract_symbols` re-downsample independently.

---

## Summary

| # | Task | Priority | Status |
|---|------|----------|--------|
| 1 | Re-downsample after frequency correction | ~~CRITICAL~~ | ✓ CLOSED - No improvement, phase rotation sufficient |
| 2 | Per-symbol frequency tweak (ctwk) | ~~CRITICAL~~ | ✓ CLOSED - Mathematically equivalent, no improvement |
| 3 | Remove redundant timing search | IMPORTANT | Pending |
| 4 | Pass downsampled signal between stages | IMPORTANT | Pending |

**Conclusion**: The hard error gap (12.4) is **not caused by frequency correction approach**. Both items 1 and 2 were tested and showed no impact. The gap likely originates from:
- LLR calculation method differences
- Symbol extraction/FFT precision
- Noise estimation approach
- Multi-symbol combining strategy

Items 3-4 are cleanup that may improve robustness but likely won't close the hard error gap.