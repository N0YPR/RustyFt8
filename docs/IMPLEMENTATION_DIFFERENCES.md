# Implementation Differences: RustyFt8 vs WSJT-X

Analysis of key differences between RustyFt8 and WSJT-X (`ft8b.f90`) that may explain the hard error gap on weak signals.

---

## TODO List (Priority Order)

### 1. [ ] Re-Downsample After Frequency Correction ⭐⭐⭐ CRITICAL

**Priority:** HIGH
**Files:** [extract.rs](../src/decode/extract.rs)
**Impact:** Most likely to close the hard error gap

#### Problem

After finding the best frequency offset, WSJT-X re-downsamples the entire signal at the corrected frequency. RustyFt8 only applies phase correction to the already-downsampled buffer.

#### WSJT-X Reference (`ft8b.f90:140-141`)

```fortran
call ft8_downsample(dd0,.false.,f1,cd0)   !Mix f1 to baseband and downsample
```

After finding `delfbest`, WSJT-X updates `f1 = f1 + delfbest` and re-downsamples.

#### Current RustyFt8 (`extract.rs:162-196`)

We only apply phase correction to the already-downsampled buffer - we never re-downsample.

#### Why This Matters

- Re-downsampling ensures the signal is **perfectly centered** at baseband
- Phase correction only **approximates** this and accumulates errors over the 12.8s signal duration
- For weak signals with frequency errors > 0.5 Hz, this is the difference between `nsync=18` and `nsync=5`

#### Evidence

| nsync Range | Hard Error Gap |
|-------------|----------------|
| ≥18 (high)  | Small (-5 to +6) |
| ≤11 (low)   | Large (+25 to +53) |

The `nsync` metric directly measures how well the signal is centered.

#### Fix

Implement proper re-downsampling at the corrected frequency instead of phase rotation.

---

### 2. [ ] Use Per-Symbol Frequency Tweak Instead of Phase Rotation ⭐⭐⭐ CRITICAL

**Priority:** HIGH
**Files:** [extract.rs](../src/decode/extract.rs)
**Impact:** Improves frequency search accuracy

#### Problem

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

#### Current RustyFt8 (`extract.rs:169-186`)

```rust
for correction_idx in -20..=20 {
    let freq_correction = correction_idx as f32 * 0.05; // ±1.0 Hz
    apply_phase_correction(&mut cd_test, freq_correction, actual_sample_rate);
    let sync = sync_downsampled(&cd_test, time_offset_samples, None, false, ...);
}
```

#### Why This Matters

- Our phase correction accumulates errors over **3200 samples**
- WSJT-X applies the tweak per-symbol (**32 samples**), resetting phase at each sync measurement
- Per-symbol approach maintains proper phase relationships throughout

#### Fix

Implement `ctwk` vector approach in `sync_downsampled` to match WSJT-X's per-symbol frequency tweak.

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

| # | Task | Priority | Estimated Impact |
|---|------|----------|------------------|
| 1 | Re-downsample after frequency correction | CRITICAL | High |
| 2 | Per-symbol frequency tweak (ctwk) | CRITICAL | Medium-High |
| 3 | Remove redundant timing search | IMPORTANT | Medium |
| 4 | Pass downsampled signal between stages | IMPORTANT | Medium |

The first two items are most likely to close the hard error gap for weak signals. Items 3-4 are cleanup that will improve overall robustness.