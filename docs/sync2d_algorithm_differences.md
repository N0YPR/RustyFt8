# Sync2D Algorithm Differences - Root Cause Analysis

**Date**: 2025-11-25
**Status**: CRITICAL DIFFERENCES IDENTIFIED

---

## Problem

F5RXL @ 1197 Hz: sync2d peaks at **bin 407 (1192.4 Hz)** instead of **bin 409 (1198.4 Hz)** - **4.6 Hz error!**

Debug output:
- Bin 407 (1192.4 Hz): sync=**14.635** ← HIGHEST (wrong!)
- Bin 408 (1195.3 Hz): sync=4.247
- Bin 409 (1198.4 Hz): sync=1.841 ← Should be highest (only 1.4 Hz off!)
- Bin 410 (1201.3 Hz): sync=1.902

This prevents coarse sync from finding the right starting frequency.

---

## Line-by-Line Comparison

### WSJT-X sync8.f90 (lines 62-74)

```fortran
do n=0,6
   m=j+jstrt+nssy*n

   ! First Costas array (symbols 0-6)
   if(m.ge.1.and.m.le.NHSYM) then
      ta=ta + s(i+nfos*icos7(n),m)              ! Add Costas tone (NO freq check!)
      t0a=t0a + sum(s(i:i+nfos*6:nfos,m))       ! Add 7 baseline bins (NO freq check!)
   endif

   ! Middle Costas array (symbols 36-42) - NO BOUNDS CHECK AT ALL!
   tb=tb + s(i+nfos*icos7(n),m+nssy*36)
   t0b=t0b + sum(s(i:i+nfos*6:nfos,m+nssy*36))

   ! Third Costas array (symbols 72-78)
   if(m+nssy*72.le.NHSYM) then
      tc=tc + s(i+nfos*icos7(n),m+nssy*72)      ! Add Costas tone (NO freq check!)
      t0c=t0c + sum(s(i:i+nfos*6:nfos,m+nssy*72))  ! Add 7 baseline bins (NO freq check!)
   endif
enddo

t=ta+tb+tc
t0=t0a+t0b+t0c
t0=(t0-t)/6.0
sync_abc=t/t0

t=tb+tc
t0=t0b+t0c
t0=(t0-t)/6.0
sync_bc=t/t0

sync2d(i,j)=max(sync_abc,sync_bc)
```

### RustyFt8 spectra.rs (lines 320-387)

```rust
for n in 0..7 {
    let m = j + jstrt + (nssy as i32) * (n as i32);
    let tone = COSTAS_PATTERN[n] as i32;

    // First Costas array (symbols 0-6)
    if m >= 0 && (m as usize) < NHSYM {           // (1) Time check
        let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
        if freq_idx < NH1 {                       // ⚠️ (2) Costas tone frequency check
            ta += spectra[freq_idx][m as usize];

            // ⚠️ BASELINE IS INSIDE freq_idx CHECK!
            for k in 0..7 {
                let baseline_idx = i + nfos * k;
                if baseline_idx < NH1 {           // ⚠️ (3) Each baseline bin checked
                    t0a += spectra[baseline_idx][m as usize];
                }
            }
        }
    }

    // Middle Costas array (symbols 36-42)
    let m2 = m + (nssy as i32) * 36;
    if m2 >= 0 && (m2 as usize) < NHSYM {         // ⚠️ Time bounds check (WSJT-X skips this!)
        let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
        if freq_idx < NH1 {                       // ⚠️ Frequency check
            tb += spectra[freq_idx][m2 as usize];
        }
        for k in 0..7 {
            let baseline_idx = i + nfos * k;
            if baseline_idx < NH1 {               // ⚠️ Frequency check
                t0b += spectra[baseline_idx][m2 as usize];
            }
        }
    }

    // Third Costas array (symbols 72-78) - similar to first
    let m3 = m + (nssy as i32) * 72;
    if m3 >= 0 && (m3 as usize) < NHSYM {
        let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
        if freq_idx < NH1 {
            tc += spectra[freq_idx][m3 as usize];
            for k in 0..7 {
                let baseline_idx = i + nfos * k;
                if baseline_idx < NH1 {
                    t0c += spectra[baseline_idx][m3 as usize];
                }
            }
        }
    }
}

let t = ta + tb + tc;
let mut t0 = t0a + t0b + t0c;
t0 = (t0 - t) / 6.0;
let sync_abc = if t0 > 0.0 { t / t0 } else { 0.0 };

let t_bc = tb + tc;
let mut t0_bc = t0b + t0c;
t0_bc = (t0_bc - t_bc) / 6.0;
let sync_bc = if t0_bc > 0.0 { t_bc / t0_bc } else { 0.0 };

sync_row[sync_idx] = sync_abc.max(sync_bc);
```

---

## Critical Differences

### Difference #1: Frequency Bounds Checking ⚠️ CRITICAL

**WSJT-X**: NO frequency bounds checking
- Assumes ia and ib are set such that i+nfos*6 < array size
- Fortran would crash or access random memory if out of bounds

**RustyFt8**: MULTIPLE frequency bounds checks
- Check 1: `if freq_idx < NH1` - Skip if Costas tone out of bounds
- Check 2: `if baseline_idx < NH1` - Skip each baseline bin if out of bounds

**Impact**: If any frequency bin is near the edge, we skip elements, reducing baseline, INFLATING sync score.

### Difference #2: Baseline Inside Frequency Check ⚠️ CRITICAL

**WSJT-X** (sync8.f90:64-67):
```fortran
if(m.ge.1.and.m.le.NHSYM) then
   ta=ta + s(i+nfos*icos7(n),m)              ! Add Costas tone
   t0a=t0a + sum(s(i:i+nfos*6:nfos,m))       ! Add baseline
endif
```
- Baseline computed if time `m` is in bounds
- Baseline computed REGARDLESS of whether Costas tone is in bounds

**RustyFt8** (spectra.rs:324-337):
```rust
if m >= 0 && (m as usize) < NHSYM {
    let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
    if freq_idx < NH1 {                       // ← Baseline INSIDE this check!
        ta += spectra[freq_idx][m as usize];
        for k in 0..7 {
            let baseline_idx = i + nfos * k;
            if baseline_idx < NH1 {
                t0a += spectra[baseline_idx][m as usize];
            }
        }
    }
}
```
- Baseline computed ONLY if Costas tone is in bounds
- If freq_idx >= NH1, we skip BOTH ta and t0a for that symbol

**Impact**: For each symbol where the Costas tone is out of bounds, we lose 7 baseline values. This makes t0 smaller, sync score LARGER.

### Difference #3: Middle Costas Bounds Checking ⚠️ CRITICAL

**WSJT-X** (sync8.f90:68-69):
```fortran
tb=tb + s(i+nfos*icos7(n),m+nssy*36)         ! NO BOUNDS CHECK AT ALL!
t0b=t0b + sum(s(i:i+nfos*6:nfos,m+nssy*36))
```
- Assumes middle Costas (symbol 36) is always in valid time range
- Safe for 15-second recordings with signals near center

**RustyFt8** (spectra.rs:342-354):
```rust
let m2 = m + (nssy as i32) * 36;
if m2 >= 0 && (m2 as usize) < NHSYM {         // ← Extra bounds check
    let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
    if freq_idx < NH1 {
        tb += spectra[freq_idx][m2 as usize];
    }
    // ... baseline ...
}
```
- Bounds-checks middle Costas time index
- Could skip middle Costas entirely for extreme negative time offsets

**Impact**: For signals with large negative time offsets, we might compute sync_bc but not sync_abc, changing which metric dominates.

---

## How This Causes 4.6 Hz Error

### Hypothesis: Frequency-Dependent Baseline Bias

Because our baseline computation is conditional:
1. **Lower frequency bins** (like bin 407): All frequencies i through i+12 are in bounds → full baseline → LOWER sync score (should be correct)
2. **Higher frequency bins** (like bin 409): Some frequencies might be handled differently → ???

Wait, this doesn't make sense for F5RXL at 1197 Hz (bin ~408), which is far from the edges.

### Alternative Hypothesis: Accumulated Rounding Errors

The conditional baseline computation could cause:
- Different numbers of baseline samples accumulated (if any bins are skipped)
- Different t0 values
- Different normalization: `t0 = (t0 - t) / 6.0`

If t0 is systematically smaller at higher frequencies, sync scores are inflated at higher frequencies, but we're seeing the OPPOSITE (lower frequencies have higher sync).

### Most Likely: Middle Costas Skipped

For signals with negative time offsets:
- F5RXL has dt=-0.77s (starts early)
- Middle Costas check: `if m2 >= 0 && (m2 as usize) < NHSYM`
- WSJT-X skips this check entirely!

If m2 goes negative for some combinations of j and n, we skip the middle Costas, which means:
- We use sync_bc (only tb+tc) instead of sync_abc (ta+tb+tc)
- The sync metric is computed differently
- Peak locations shift!

---

## Fix Strategy

### Option 1: Remove All Frequency Bounds Checks (Match WSJT-X Exactly)

**Changes**:
1. Remove `if freq_idx < NH1` checks
2. Remove `if baseline_idx < NH1` checks
3. Remove `if m2 >= 0 && (m2 as usize) < NHSYM` check for middle Costas
4. Trust that ia and ib are set correctly to prevent out-of-bounds

**Pros**: Exact match with WSJT-X behavior

**Cons**: Could panic if ia/ib not set correctly (but WSJT-X has same issue)

### Option 2: Move Baseline Outside Frequency Check

**Changes**:
1. Keep time bounds checks
2. Remove frequency bounds checks on baseline (trust ia/ib)
3. Compute baseline for ALL 7 frequency bins regardless of Costas tone bounds

**Pros**: Safer (keeps time bounds checks), closer to WSJT-X

**Cons**: Still doesn't perfectly match WSJT-X

### Option 3: Add Debug Output First

**Before fixing**, add debug output to verify:
1. Are any freq_idx >= NH1 for F5RXL?
2. Is m2 < 0 for any combinations?
3. How many baseline values are accumulated for bins 407 vs 409?

**Pros**: Understand root cause before fixing

---

## Recommendation

**Implement Option 3 first**: Add debug output for F5RXL to see:
- Which bounds checks are triggering
- How many baseline values accumulated for each bin
- Whether middle Costas is being skipped

Then implement **Option 1** (exact WSJT-X match) if debug confirms bounds checks are the issue.

---

## Constants

- NFFT1 = 4096
- NSPS = 1920
- NH1 = NFFT1/2 = 2048
- nfos = NFFT1/NSPS = 2 (integer division)
- nssy = NSPS/NSTEP = 1920/480 = 4
- COSTAS_PATTERN = [3,1,4,0,6,5,2]

For F5RXL @ bin 408:
- Frequencies checked: 408, 408+2, 408+4, 408+6, 408+8, 408+10, 408+12 = 408-420
- All well within NH1=2048 ✓
- So frequency bounds shouldn't be the issue for F5RXL specifically

---

## Next Steps

1. ✅ Document differences (this file)
2. ⏳ Add debug output to spectra.rs for F5RXL
3. ⏳ Run test and analyze why bin 407 has higher sync than bin 409
4. ⏳ Implement Option 1 (remove bounds checks to match WSJT-X)
5. ⏳ Test and verify 22/22 decodes

---

## References

- WSJT-X sync8.f90 lines 56-84: Costas correlation implementation
- RustyFt8 src/sync/spectra.rs lines 281-399: compute_sync2d function
- [interpolation_results_20251125.md](interpolation_results_20251125.md): Discovery that sync2d peaks at wrong bins
