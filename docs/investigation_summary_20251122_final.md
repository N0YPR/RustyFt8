# FT8 Decoder Investigation - Final Summary
**Date**: 2025-11-22
**Status**: 11/22 messages (50%), major bug fixed, LLR quality improved 4x

---

## Critical Bug Fixed: Out-of-Bounds Symbol Extraction

### Root Cause
Signals with **negative time offsets** (dt < 0) caused start_offset to be negative, leading to out-of-bounds symbol extraction. The first 0-4 symbols were being ZEROED, which is catastrophic for multi-symbol combining:

- **nsym=1**: 3 bits corrupted per zeroed symbol
- **nsym=2**: 6 bits corrupted (entire pair)
- **nsym=3**: 9 bits corrupted (entire triplet)

### Impact
- **49 candidates** had negative dt_out
- **Example (2191.8 Hz)**:
  - Before: nsym=2 mean_abs_LLR=0.43 (5.5x degradation!)
  - After:  nsym=2 mean_abs_LLR=1.76 (4x improvement!)

### Fix
Added bounds clipping in `src/sync/extract.rs`:
```rust
let min_offset = 0i32;
let max_offset = cd.len() as i32 - (NN as i32 * nsps_down as i32);

if start_offset < min_offset {
    start_offset = min_offset; // Clip to valid range
}
```

---

## Investigation Progress

### Session Accomplishments

1. ✅ **Enabled comprehensive LDPC diagnostics**
   - 65k lines of BP iteration logging
   - Identified convergence patterns (0-16 iters success, or stuck)

2. ✅ **Verified LDPC implementation correctness**
   - Matched WSJT-X algorithm
   - Verified LLR normalization (divide by std_dev, scale by 2.83)
   - Verified Gray code mapping

3. ✅ **Tested OSD parameter variations**
   - Increased order 2 → 4: No improvement
   - Conclusion: OSD not the bottleneck

4. ✅ **Identified multi-symbol combining degradation**
   - nsym=2/3 showed 5.5x LLR degradation
   - Root caused to out-of-bounds symbols

5. ✅ **Fixed bounds error**
   - LLR quality improved 4x for nsym=2
   - Prevented symbol zeroing

6. ✅ **Added phase correction logging**
   - Corrections found in ±0.3 Hz range
   - Improvements minimal (0.1-12%)

### Key Findings

| Finding | Impact |
|---------|--------|
| **BP convergence is binary** | Either succeeds in 0-16 iters or gets stuck |
| **Initial parity state critical** | Need 0-32 failing checks initially to converge |
| **Bounds error catastrophic for nsym=2/3** | Fixed, improved LLRs 4x |
| **LLR threshold is ~2.5-2.7** | Below this, initial parity state too poor |

---

## Current Status: 11/22 Messages (50%)

### What Works ✅
- Candidate generation (all signals found)
- Fine synchronization (frequencies refined correctly)
- Symbol extraction (mostly working, bounds fixed)
- LDPC BP algorithm (correct implementation)
- OSD fallback (correct implementation)

### What's Still Failing ❌
- **LLR quality insufficient** (mean=1.76-2.3 vs needed ~2.7)
- **Initial parity check state poor** (38/83 failing for K1BZM)
- **Some signals still don't decode** despite improved LLRs

---

## Remaining Issues

### 1. LLR Quality Still Below Threshold

Even with bounds fix:
- nsym=2: mean_abs_LLR=1.76 (better, but still < 2.5 needed)
- nsym=1: mean_abs_LLR=2.06-2.38 (close, but initial parity state poor)

**Hypothesis**: Phase coherence still not perfect, or symbol timing slightly off

### 2. Phase Correction Limited

Current: ±0.3 Hz search range, 0.05 Hz steps
- Improvements seen: 0.1-12% sync boost
- May need wider range or finer resolution

### 3. Symbol Timing Precision

- nsps_down = 32 samples/symbol at 200 Hz
- FFT window alignment may need tuning
- Start offset search only ±10 samples (±50ms)

---

## Next Steps (Priority Order)

### 🔴 Priority 1: Improve Phase Coherence

**Current**: ±0.3 Hz search, minimal improvement
**Try**:
1. Widen search range to ±1.0 Hz
2. Use finer resolution (0.02 Hz steps)
3. Test adaptive phase tracking (per-symbol correction)

### 🟡 Priority 2: Verify Symbol Timing

**Check**:
1. nsps_down calculation (should be 29, we have 32?)
2. FFT window offset (currently 0)
3. Start offset optimization (currently ±10 samples)

### 🟢 Priority 3: LLR Computation Details

**Investigate**:
1. cs[][] normalization factor (1000.0) - is this correct?
2. s8[][] vs cs[][] - are magnitudes computed correctly?
3. Complex amplitude extraction from FFT bins

### ⚪ Priority 4: Compare with WSJT-X Symbol Extraction

**Study**: `wsjtx/lib/ft8/ft8b.f90` lines 150-200
- How they extract complex symbols
- Their phase correction (twkfreq1)
- Their normalization approach

---

## Test Results Timeline

| Milestone | Messages | Key Change |
|-----------|----------|------------|
| Baseline | 8/19 (42%) | Initial state |
| Time penalty | 10/19 (53%) | Candidate selection improved |
| Norm fix | 11/22 (50%) | Fixed downsample bug |
| Pipeline verified | 11/22 (50%) | Isolated to LDPC |
| OSD order→4 | 11/22 (50%) | No improvement |
| **Bounds fix** | **11/22 (50%)** | **LLR quality +4x** |
| **Target** | **22/22 (100%)** | Match WSJT-X |

---

## Documentation Created

1. `SESSION_SUMMARY.md` - Complete original session summary
2. `docs/ldpc_convergence_analysis.md` - BP convergence patterns
3. `docs/investigation_20251122_continued.md` - Investigation continuation
4. `docs/investigation_summary_20251122_final.md` - This file

---

## Key Code Changes

### src/sync/extract.rs

**Lines 222-238**: Bounds clipping
```rust
let min_offset = 0i32;
let max_offset = cd.len() as i32 - (NN as i32 * nsps_down as i32);

if start_offset < min_offset {
    eprintln!("    CLIPPING start_offset: {} -> {}", start_offset, min_offset);
    start_offset = min_offset;
}
```

**Lines 182-188**: Phase correction logging
```rust
if best_correction.abs() > 0.001 {
    eprintln!("    Phase correction: {:.3} Hz (sync: {:.3} -> {:.3})",
             best_correction, initial_sync, best_sync);
}
```

**Lines 245-252**: Out-of-bounds warning
```rust
if i1 < 0 || (i1 as usize + nsps_down) > cd.len() {
    eprintln!("    WARNING: Symbol {} out of bounds!", k);
    // Set to zero
}
```

### src/ldpc/mod.rs

**Line 64**: OSD order increased (tested, no improvement)
```rust
let osd_order = 4; // Was 2
```

---

## Statistics

- **Investigation duration**: ~4 hours
- **Lines of diagnostic output**: 65,951
- **Commits**: 3 (downsample fix, investigation docs, bounds fix)
- **LLR improvement**: 4x for nsym=2 (0.43 → 1.76)
- **Candidates with negative dt**: 49/150 (33%)

---

## Success Metrics (Updated)

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Decode rate | 11/22 (50%) | 22/22 (100%) | ❌ In progress |
| LLR quality (nsym=2) | 1.76 | ~2.7 | ⚠️ Improved but insufficient |
| Bounds errors | 0 (fixed) | 0 | ✅ Complete |
| Multi-symbol degradation | 4x improvement | No degradation | ⚠️ Better but not solved |

---

## Conclusion

### Major Progress
- ✅ Fixed critical bounds error affecting 33% of candidates
- ✅ Improved LLR quality 4x for multi-symbol combining
- ✅ Comprehensive diagnostics and documentation created
- ✅ Root cause analysis complete

### Remaining Work
The decoder is **close** but not yet matching WSJT-X performance. The LLR quality improved dramatically (0.43 → 1.76) but needs to reach ~2.7 for reliable LDPC convergence. The most promising next steps are:

1. **Widen phase correction search** (±1.0 Hz instead of ±0.3 Hz)
2. **Verify symbol timing** (check nsps_down=32 vs expected 29)
3. **Compare symbol extraction** with WSJT-X implementation details

The investigation has successfully isolated the problem to **symbol extraction quality**, specifically phase coherence and timing precision.

---

## Quick Test Command

```bash
# Run test with all diagnostics
cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored --nocapture 2>&1 | tee test_output.txt

# Check LLR quality for 2191.8 Hz
grep -A 2 "EXTRACT: freq=2191.8 Hz" test_output.txt

# Check bounds clipping
grep "CLIPPING" test_output.txt | wc -l

# Compare with WSJT-X
wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 -d 3 tests/test_data/210703_133430.wav
```
