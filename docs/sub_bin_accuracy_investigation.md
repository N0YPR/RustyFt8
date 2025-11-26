# Sub-Bin Frequency Accuracy Investigation - 2025-11-25

## Question

How does WSJT-X achieve sub-bin frequency accuracy to decode signals like F5RXL @ 1197 Hz, when we're getting 1196.8 Hz (0.2 Hz off)?

## Investigation Results

### Finding 1: Downsampling Resolution ✅ MATCHED

**WSJT-X** (ft8_downsample.f90:6):
```fortran
parameter (NFFT1=192000, NFFT2=3200)
df = 12000.0 / 192000 = 0.0625 Hz per bin
```

**RustyFt8** (src/sync/downsample.rs:28-29):
```rust
const NFFT_IN: usize = 192000;
const NFFT_OUT: usize = 3200;
df = 12000.0 / 192000 = 0.0625 Hz per bin
```

**Conclusion**: We have identical 0.0625 Hz downsampling resolution. Both can center on any frequency with 0.0625 Hz accuracy.

### Finding 2: Fine Sync Discrete Search ✅ MATCHED

**WSJT-X** (ft8b.f90:120-133):
```fortran
do ifr=-5,5                              !Search over +/- 2.5 Hz
  delf=ifr*0.5                           !0.5 Hz steps
  ...
```

**RustyFt8** (src/sync/fine.rs:193-211):
```rust
for df in -5..=5 {
    let freq_offset = df as f32 * 0.5;  // 0.5 Hz steps
    ...
```

**Conclusion**: Both search in discrete 0.5 Hz steps. No interpolation found in WSJT-X code.

### Finding 3: Coarse Sync Resolution - QUANTIZATION IDENTIFIED

**WSJT-X** (sync8.f90:122):
```fortran
candidate0(1,k)=n*df   ! No interpolation, uses bin center
```

**Coarse sync FFT** (both implementations):
```
NFFT1 = 4096
df = 12000 / 4096 = 2.93 Hz per bin
```

**F5RXL frequency bins**:
- Bin 408: 408 × 2.93 = **1195.4 Hz** ← What RustyFt8 finds
- Bin 409: 409 × 2.93 = **1198.4 Hz** ← What WSJT-X likely finds
- Actual signal: **1197.0 Hz** (between bins)

**Conclusion**: Coarse sync has 2.93 Hz quantization. No interpolation in WSJT-X.

### Finding 4: No Parabolic Interpolation in WSJT-X

Searched WSJT-X FT8 code for:
- `parabola`, `interpol`, `peak.*fit`, `quadratic`: **No matches**

**Conclusion**: WSJT-X does NOT use parabolic interpolation or any explicit sub-bin refinement.

## The Real Answer: WSJT-X Gets Lucky (or We're Unlucky)

### Why WSJT-X Reports 1197 Hz

WSJT-X probably finds F5RXL at:
- **Coarse sync**: bin 409 = 1198.4 Hz (closer to 1197 Hz than our 1195.4 Hz)
- **Fine sync**: Tests [1195.9, 1196.4, 1196.9, **1197.4**, 1197.9, 1198.4, 1198.9, 1199.4, 1199.9, 1200.4, 1200.9]
- **Best match**: 1197.4 Hz (0.4 Hz off) or 1196.9 Hz (0.1 Hz off)
- **Reported**: 1197 Hz (rounded to nearest integer)

### Why RustyFt8 Finds 1196.8 Hz

We find F5RXL at:
- **Coarse sync**: bin 408 = 1195.4 Hz (farther from 1197 Hz)
- **Fine sync**: Tests [1192.9, 1193.4, 1193.9, 1194.4, 1194.9, 1195.4, 1195.9, 1196.4, **1196.9**, 1197.4, 1197.9]
- **Best match**: 1196.8 Hz (actual, after final refinement)
- **Error**: 0.2 Hz off

### The Key Difference: Coarse Sync Starting Point

**Hypothesis**: WSJT-X's sync2d computation (or baseline normalization, or Costas correlation) produces slightly different power distribution, causing bin 409 to have higher sync score than bin 408.

**Result**:
- WSJT-X starts at 1198.4 Hz → fine sync finds ~1197.4 Hz → 0.4 Hz error
- RustyFt8 starts at 1195.4 Hz → fine sync finds 1196.8 Hz → 0.2 Hz error

**Ironically**: Our fine sync might be MORE accurate (0.2 Hz error vs 0.4 Hz error), but starting from the wrong coarse bin makes us fail!

## Why 0.2 Hz Error Matters

FT8 tone extraction:
- FFT resolution: 6.25 Hz / 32 bins = **0.195 Hz per bin**
- 0.2 Hz error = **~1 FFT bin offset**
- Wrong tone bins get more power than correct bins
- Result: 20% tone errors → 28% bit error rate → exceeds LDPC's ~20% correction capability

## Solutions

### Option 1: Fix Coarse Sync to Match WSJT-X

**Goal**: Make coarse sync find bin 409 (1198.4 Hz) instead of bin 408 (1195.4 Hz)

**Investigation needed**:
1. Compare sync2d values at bins 408 vs 409
2. Check if baseline normalization differs
3. Verify Costas correlation computation
4. Test if different averaging methods affect peak location

**Expected impact**: If coarse sync finds the right bin, fine sync should find ~1197 Hz

### Option 2: Parabolic Interpolation in Coarse Sync

**Goal**: Refine coarse sync bin to sub-bin accuracy before fine sync

**Implementation**:
1. After finding peak bin `n`, get sync2d[n-1], sync2d[n], sync2d[n+1]
2. Fit parabola: `f(x) = ax² + bx + c`
3. Find peak: `refined_freq = (n + dx) * df` where `dx = -b/(2a)`
4. Pass refined frequency to fine sync

**Expected accuracy**: ±0.5 bins = ±1.5 Hz → after fine sync: ±0.1-0.2 Hz

**Advantage**: Doesn't require matching WSJT-X's sync2d exactly

### Option 3: Parabolic Interpolation in Fine Sync

**Goal**: Refine fine sync frequency after discrete 0.5 Hz search

**Implementation**:
1. After discrete search, save sync scores at [best_freq-0.5, best_freq, best_freq+0.5]
2. Fit parabola to 3 points
3. Find peak: `refined_freq = best_freq + dx` where `dx = -b/(2a)`
4. Use interpolated frequency for downsampling and extraction

**Expected accuracy**: ±0.1-0.2 Hz improvement over discrete search

**Advantage**: Orthogonal to coarse sync, works even if coarse sync is off

### Option 4: Finer FFT in Coarse Sync

**Goal**: Increase coarse sync resolution from 2.93 Hz to ~0.5 Hz

**Implementation**:
- Change NFFT1 from 4096 to 24576 (6x larger)
- New resolution: 12000 / 24576 = 0.49 Hz per bin
- Directly matches fine sync's 0.5 Hz steps

**Cost**: 6x more memory and compute for coarse sync
**Benefit**: Eliminates coarse sync quantization entirely

## Recommendation

**Implement Option 2 (coarse sync interpolation) + Option 3 (fine sync interpolation)**

**Rationale**:
1. Option 2 fixes the immediate problem (wrong coarse bin)
2. Option 3 refines further to handle noisy sync curves
3. Combined: Should achieve <0.1 Hz accuracy consistently
4. Doesn't require massive FFT size increase (Option 4)
5. Doesn't depend on matching WSJT-X's exact sync2d quirks (Option 1)

**Expected result**: F5RXL and similar signals decode successfully

## References

- WSJT-X ft8b.f90 lines 120-133: Fine sync frequency search
- WSJT-X ft8_downsample.f90 lines 1-51: Downsampling with 0.0625 Hz resolution
- WSJT-X sync8.f90 lines 115-125: Coarse sync candidate generation
- [tone_extraction_root_cause.md](tone_extraction_root_cause.md): Analysis of 0.2-0.3 Hz errors causing 20% tone errors
- [session_20251125_part8_sync_fix_results.md](session_20251125_part8_sync_fix_results.md): F5RXL case study
