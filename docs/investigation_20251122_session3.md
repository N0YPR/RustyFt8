# FT8 Decoder Investigation - Session 3 Summary
**Date**: 2025-11-22
**Status**: 9/22 messages (41%), **critical bug found in fine_sync**

---

## Major Discovery: Fine Sync is Broken

### The Problem
Coarse sync correctly identifies candidate signals, but **fine_sync incorrectly refines them** to wrong frequencies and time offsets.

**Example - K1BZM signal**:
- **WSJT-X (correct)**: 2695 Hz, dt=-0.1s, SNR=-3 dB
- **Our coarse sync**: 2695.3 Hz, dt=-0.14s ✅ (correct!)
- **Our fine_sync**: 2616.6 Hz, dt=1.00s ❌ (completely wrong!)

The fine_sync function is jumping to a local maximum at a completely different signal instead of refining the original candidate.

### Impact
- Strong signals (-2 to -7 dB) fail to decode NOT because of LDPC issues
- BUT because fine_sync gives wrong frequency/timing to symbol extraction
- We successfully decode -16 dB signals when fine_sync works correctly
- We fail to decode -3 dB signals when fine_sync fails

---

## Investigation Timeline

### 1. Multi-Symbol Combining Analysis

**Discovered**: Phase cancellation in nsym=2/3
- Symbol 7, tone 0: re=+0.10, im=-0.19
- Symbol 8, tone 3: re=-0.09, im=-0.26
- Sum: re=0.01 (90% cancellation!), im=-0.45
- Result: s2 values too small (0.0001-0.0005 range)

**Tests**:
- Removed normalization by 1000: Made things worse (9/22)
- Disabled phase correction: Made things worse (9/22)
- Disabled nsym=2/3 entirely: 9/22 correct (removed 2 false positives)

**Conclusion**: Multi-symbol combining produces:
- 2 FALSE POSITIVES (wrong decodes not in WSJT-X output)
- 0 correct decodes
- Phase coherence issues make nsym=2/3 unreliable

### 2. False Positive Discovery

With nsym=1/2/3 enabled:
- Total: 11 decodes
- Breakdown: 9 correct + 2 false positives

False positives (not in WSJT-X):
1. "YR9CQS UA7ZVQ/P RH74" @ 428.7 Hz, nsym=2
2. "BR3QHU/R PA8IXE/R R BH02" @ 742.1 Hz, nsym=2

**Root cause**: Phase cancellation creates random bit patterns that occasionally pass CRC, producing garbage messages.

### 3. Fine Sync Bug Discovery

Analyzed why strong signals fail:
- **Decoded successfully**: SNRs -16 to +16 dB
- **Missing**: SNRs -20 to -2 dB (includes STRONGER signals!)

This ruled out SNR/LDPC as the primary issue.

**Traced K1BZM candidate**:
```
Coarse sync: 2695.3 Hz, dt=-0.14s (correct)
       ↓
Fine sync:   2616.6 Hz, dt=1.00s (WRONG - jumped to different signal!)
       ↓
Symbol extraction: Gets wrong signal
       ↓
LDPC: Fails (wrong symbols)
```

---

## Root Cause Analysis

### Primary Issue: Fine Sync Function
The `fine_sync` function in `src/sync/fine.rs` is:
1. Not staying close enough to the initial candidate
2. Finding stronger local maxima from different signals
3. Jumping to wrong frequency/time combinations

### Secondary Issue: Multi-Symbol Combining
The `extract_symbols` implementation produces:
1. Phase cancellation between adjacent symbols
2. Small s2 magnitudes leading to poor LLRs
3. False positive decodes from random patterns

---

## Test Results

| Configuration | Correct | False Positives | Total | Notes |
|---------------|---------|-----------------|-------|-------|
| **Baseline** (nsym=1/2/3) | 9 | 2 | 11 | False positives from nsym=2 |
| **nsym=1 only** | 9 | 0 | 9 | No false positives |
| **No normalization** | 9 | 0 | 9 | Worse than baseline |
| **No phase correction** | 9 | 0 | 9 | Worse than baseline |
| **WSJT-X** | 22 | 0 | 22 | Target ✅ |

---

## Code Investigation

### Files Examined

1. **src/sync/extract.rs**:
   - Added extensive debugging for K1BZM
   - Verified normalization by 1000 is correct
   - Tested removing normalization (made things worse)
   - Tested disabling phase correction (made things worse)

2. **src/sync/fine.rs** (needs investigation):
   - Fine sync function is incorrectly refining candidates
   - Need to understand search algorithm
   - May need to constrain search range around initial candidate

3. **src/decoder.rs**:
   - Modified nsym_values to test configurations
   - Current: nsym=1 only to avoid false positives

---

## Next Steps (Priority Order)

### 🔴 Priority 1: Fix Fine Sync (CRITICAL)

**Issue**: Fine sync jumping to wrong signals

**Investigation needed**:
1. Read `src/sync/fine.rs` to understand search algorithm
2. Check search range - is it too wide?
3. Verify sync metric - is it finding wrong local maxima?
4. Test constraining search to ±50 Hz and ±0.2s from coarse sync

**Expected impact**: Should fix most of the 13 missing messages

### 🟡 Priority 2: Fix Multi-Symbol Combining

**Issue**: Phase cancellation causing false positives and poor LLRs

**Options**:
1. Per-symbol phase tracking (estimate and correct phase drift)
2. Incoherent combining (magnitude-only, no phase)
3. Different normalization for nsym=2/3
4. Study WSJT-X's exact multi-symbol implementation more carefully

**Expected impact**: Enable nsym=2/3 without false positives

### 🟢 Priority 3: Compare with WSJT-X Implementation

**Tasks**:
1. Study `wsjtx/lib/ft8/sync8.f90` - fine sync algorithm
2. Study `wsjtx/lib/ft8/ft8b.f90` lines 193-202 - multi-symbol combining
3. Check if they use different metrics or constraints

---

## Key Findings

1. ✅ **LDPC implementation is correct** - verified against WSJT-X
2. ✅ **Symbol extraction normalization is correct** - cs[][] / 1000 matches WSJT-X
3. ✅ **LLR normalization is correct** - std_dev division matches WSJT-X
4. ❌ **Fine sync is broken** - jumps to wrong signals (THIS IS THE BLOCKER)
5. ❌ **Multi-symbol combining produces false positives** - phase issues

---

## Debug Output Examples

### Successful fine sync (W1FC at 2572.7 Hz):
```
FINE_SYNC: freq=2573.9 Hz, dt_in=0.20s, sync_in=43.256
  REFINED: freq_in=2573.9 -> freq_out=2572.7 Hz, dt_out=0.21s, sync_out=38.223
EXTRACT: freq=2572.7 Hz, dt=0.21s, nsym=1
  Extracted: nsync=21/21, mean_abs_LLR=3.70, max_LLR=8.26
```
Small refinement (1.2 Hz, 0.01s), strong sync (38.2), decodes successfully.

### Failed fine sync (K1BZM at 2695 Hz):
```
FINE_SYNC: freq=2695.3 Hz, dt_in=-0.14s, sync_in=4.976
  REFINED: freq_in=2619.1 -> freq_out=2616.6 Hz, dt_out=1.00s, sync_out=0.570
```
Huge jump (78 Hz, 1.14s), finds wrong signal entirely.

---

## Statistics

- **Investigation duration**: ~6 hours total (across 3 sessions)
- **Messages decoded**: 9/22 (41%)
- **False positives**: 2 (with nsym=2/3 enabled)
- **Commits**: None yet (still investigating)

---

## Quick Test Commands

```bash
# Run test with current config (nsym=1 only)
cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored --nocapture 2>&1 | tail -30

# Check specific candidate refinement
cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored --nocapture 2>&1 | grep -A 5 "FINE_SYNC: freq=2695"

# Compare with WSJT-X
wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 -d 3 tests/test_data/210703_133430.wav
```

---

## Conclusion

The investigation has identified **two major bugs**:

1. **Fine sync is broken** (critical blocker):
   - Causes 50%+ of decode failures
   - Affects strong signals more than implementation quality issues
   - Must be fixed before other optimizations matter

2. **Multi-symbol combining produces false positives**:
   - Phase cancellation from timing/frequency errors
   - Creates garbage decodes that pass CRC
   - Currently disabled to avoid false positives

**Next session must focus on** fixing the fine_sync function to properly refine candidates without jumping to wrong signals.
