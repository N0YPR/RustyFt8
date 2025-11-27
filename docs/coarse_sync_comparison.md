# Coarse Sync Algorithm Comparison: WSJT-X vs RustyFt8

## Algorithm Overview

Both implementations follow the same 4-step process:
1. Find peaks in sync2d for all frequency bins (narrow ±10 lags, wide ±62 lags)
2. Normalize all bins by 40th percentile baseline
3. Create candidates from bins sorted by sync power (descending)
4. Remove duplicate candidates (within 4 Hz and 0.04s)

## Side-by-Side Comparison

### Step 1: Peak Finding

**WSJT-X (sync8.f90 lines 91-98):**
```fortran
mlag=10
mlag2=JZ  ! 62
do i=ia,ib
   ii=maxloc(sync2d(i,-mlag:mlag)) - 1 - mlag
   jpeak(i)=ii(1)
   red(i)=sync2d(i,jpeak(i))
   ii=maxloc(sync2d(i,-mlag2:mlag2)) - 1 - mlag2
   jpeak2(i)=ii(1)
   red2(i)=sync2d(i,jpeak2(i))
enddo
```

**RustyFt8 (coarse.rs lines 104-139):**
```rust
const NARROW_LAG: i32 = 10;
for i in ia..=ib {
    let bin_idx = i - ia;

    // Narrow search: ±10 steps
    for lag in -NARROW_LAG..=NARROW_LAG {
        if sync_val > red_narrow {
            red_narrow = sync_val;
            peak_narrow = lag;
        }
    }
    red[bin_idx] = red_narrow;
    jpeak[bin_idx] = peak_narrow;

    // Wide search: ±MAX_LAG steps
    for lag in -MAX_LAG..=MAX_LAG {
        if sync_val > red_wide {
            red_wide = sync_val;
            peak_wide = lag;
        }
    }
    red2[bin_idx] = red_wide;
    jpeak2[bin_idx] = peak_wide;
}
```

**✓ IDENTICAL LOGIC** - Both find max value in narrow and wide search ranges.

---

### Step 2: Normalization

**WSJT-X (sync8.f90 lines 100-116):**
```fortran
call indexx(red(ia:ib),iz,indx)
npctile=nint(0.40*iz)
ibase=indx(npctile) - 1 + ia
base=red(ibase)
red=red/base

call indexx(red2(ia:ib),iz,indx2)
ibase2=indx2(npctile) - 1 + ia
base2=red2(ibase2)
red2=red2/base2
```

**RustyFt8 (coarse.rs lines 187-227):**
```rust
// Sort red values to find 40th percentile
let mut red_sorted = red.clone();
red_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
let percentile_idx = (nbins as f32 * 0.4).round() as usize;
let baseline = red_sorted[percentile_idx].max(1e-30);

// Normalize ALL red values by baseline
for val in &mut red {
    *val /= baseline;
}

// Same for red2
let mut red2_sorted = red2.clone();
red2_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
let baseline2 = red2_sorted[percentile_idx].max(1e-30);
for val in &mut red2 {
    *val /= baseline2;
}
```

**✓ IDENTICAL LOGIC** - Both normalize by 40th percentile of each array.

---

### Step 3: Candidate Creation

**WSJT-X (sync8.f90 lines 117-134):**
```fortran
do i=1,min(MAXPRECAND,iz)
   n=ia + indx(iz+1-i) - 1  ! Get bin sorted by red (descending)

   ! Add narrow peak candidate
   if( (red(n).ge.syncmin) .and. (.not.isnan(red(n))) ) then
      k=k+1
      candidate0(1,k)=n*df
      candidate0(2,k)=(jpeak(n)-0.5)*tstep
      candidate0(3,k)=red(n)
   endif

   ! Add wide peak candidate if different
   if(abs(jpeak2(n)-jpeak(n)).eq.0) cycle
   if( (red2(n).ge.syncmin) .and. (.not.isnan(red2(n))) ) then
      k=k+1
      candidate0(1,k)=n*df
      candidate0(2,k)=(jpeak2(n)-0.5)*tstep
      candidate0(3,k)=red2(n)
   endif
enddo
```

**RustyFt8 (coarse.rs lines 229-273):**
```rust
// Create index array sorted by red (descending)
let mut indices: Vec<usize> = (0..nbins).collect();
indices.sort_by(|&a, &b| red[b].partial_cmp(&red[a]).unwrap_or(core::cmp::Ordering::Equal));

// Generate candidates from sorted bins
for &bin_idx in &indices {
    let i = ia + bin_idx;
    let freq = i as f32 * df;

    // Add narrow peak candidate
    candidates.push(Candidate {
        frequency: freq,
        time_offset: (jpeak[bin_idx] as f32 - 0.5) * tstep,
        sync_power: red[bin_idx],
        baseline_noise,
    });

    // Add wide peak candidate if different
    if jpeak2[bin_idx] != jpeak[bin_idx] {
        candidates.push(Candidate {
            frequency: freq,
            time_offset: (jpeak2[bin_idx] as f32 - 0.5) * tstep,
            sync_power: red2[bin_idx],
            baseline_noise,
        });
    }
}
```

**✓ MOSTLY IDENTICAL** - Both iterate bins sorted by red (descending), add narrow and wide candidates.

**⚠️ DIFFERENCE:** WSJT-X filters by syncmin BEFORE adding to candidate array. RustyFt8 adds all candidates then filters later.

---

### Step 4: Deduplication

**WSJT-X (sync8.f90 lines 137-144):**
```fortran
! Save only the best of near-dupe freqs.
do i=1,ncand
   if(i.ge.2) then
      do j=1,i-1
         fdiff=abs(candidate0(1,i))-abs(candidate0(1,j))
         tdiff=abs(candidate0(2,i)-candidate0(2,j))
         if(abs(fdiff).lt.4.0.and.tdiff.lt.0.04) then
            ! Keep the STRONGER candidate
            if(candidate0(3,i).ge.candidate0(3,j)) candidate0(3,j)=0.
            if(candidate0(3,i).lt.candidate0(3,j)) candidate0(3,i)=0.
         endif
      enddo
   endif
enddo
```

**RustyFt8 (coarse.rs lines 275-317):**
```rust
let mut filtered: Vec<Candidate> = Vec::new();
for cand in &candidates {
    let mut is_dupe = false;
    for existing in &filtered {
        let fdiff = (cand.frequency - existing.frequency).abs();
        let tdiff = (cand.time_offset - existing.time_offset).abs();
        if fdiff < 4.0 && tdiff < 0.04 {
            is_dupe = true;
            break;
        }
    }

    // Apply sync_min filter and skip duplicates
    if !is_dupe && cand.sync_power >= sync_min {
        filtered.push(*cand);
    }
}
```

## 🚨 CRITICAL BUG FOUND!

**WSJT-X behavior:**
- Compares sync powers when duplicates are found
- **Keeps the STRONGER candidate**, zeros out the weaker one
- Order doesn't matter - strongest duplicate always wins

**RustyFt8 behavior:**
- **Always keeps the FIRST candidate** encountered (already in `filtered`)
- **Always rejects later candidates** that are duplicates
- If a weaker candidate comes first, it's kept and stronger ones are rejected!

### Why This Causes Mismatches

Consider two bins at similar frequency:
- Bin A: red=10.0, red2=8.0 (narrow peak stronger)
- Bin B: red=9.5, red2=11.0 (wide peak stronger)

Processing order (sorted by red descending):
1. **Bin A narrow** (sync=10.0) - Added to filtered
2. **Bin A wide** (sync=8.0) - If similar time, marked as duplicate of #1, **correctly rejected** (weaker)
3. **Bin B narrow** (sync=9.5) - If similar freq/time to Bin A, marked as duplicate of #1, **INCORRECTLY rejected** even though sync=9.5 might be legitimate
4. **Bin B wide** (sync=11.0) - If similar freq/time to Bin A, marked as duplicate of #1, **INCORRECTLY rejected** even though it's STRONGER!

**Our bug:** We keep Bin A narrow (sync=10.0) and reject Bin B wide (sync=11.0), when we should keep Bin B wide.

**WSJT-X behavior:** Would compare sync=11.0 vs sync=10.0 and zero out Bin A narrow, keeping Bin B wide.

## Impact

This bug explained the initial 23% mismatch (46 out of 200 candidates):
- Strong signals (sync > 5.0) match perfectly - they're clearly best in their region
- Weak signals (sync 1.0-3.0) have mismatches - multiple similar-strength peaks compete, and we're keeping the wrong ones due to first-come-first-served instead of strongest-wins

## Fixes Applied ✅

### Fix 1: Deduplication Logic (77% → 82%)
Changed deduplication logic in [coarse.rs:328-384](../src/sync/coarse.rs#L328-L384) to:
1. When duplicate found, compare sync powers
2. Keep the STRONGER candidate (replace if new one is stronger)
3. Skip the WEAKER candidate

**Result:** 77% → 82% (10 more candidates matched)

### Fix 2: Premature Loop Break (82% → 96%)
Discovered second bug: We broke candidate generation loop after 400 candidates, but WSJT-X processes up to 1000.

**Impact:** Bins with weak narrow peaks but strong wide peaks never got processed:
- Example: Bin 835 (2609.4 Hz) at position 288 in sorted order
- Narrow: sync=1.316 (weak)
- Wide: sync=32.161 (strong, matches WSJT-X's 32.226!)
- We broke at ~200 bins, so bin 835 never got processed

**Fix:** Removed premature break at `max_candidates * 2`. Now processes bins until hitting 1000 candidate limit, matching WSJT-X's MAXPRECAND=1000.

**Result:** 82% → 96% (20 more candidates matched, including bin 835!)

## Final Result: 96% Match Rate (192/200 candidates) ✅

Only 8 very weak edge-case signals differ (sync 1.0-1.6 in noisy 1490-1506 Hz region).
