# LDPC Investigation Continuation - 2025-11-22

## Summary of Continued Investigation

### Actions Taken

1. **Enabled detailed LDPC diagnostics** (65k+ lines of output)
   - Tracked BP convergence iteration-by-iteration
   - Identified exact failure patterns

2. **Analyzed BP convergence behavior**:
   - **Successful decodes**: Converge in 0-16 iterations (mostly 0-2)
   - **Failed decodes**: Get stuck at various ncheck values (10-44/83 failing)
   - **K1BZM example**: Progresses from 38/83 → 10/83, then STUCK

3. **Compared with WSJT-X implementation**:
   - ✅ Algorithmically identical BP decoder
   - ✅ LLR normalization matches (divide by std dev, scale by 2.83)
   - ✅ Gray code mapping verified correct
   - WSJT-X has early stopping criterion (efficiency, not correctness)

4. **Tested OSD parameter variations**:
   - Increased OSD order: 2 → 4
   - **Result**: No improvement (still 11/22)
   - Conclusion: OSD is not the bottleneck

---

## Key Findings

### 1. Initial Parity Check State is Critical

| Signal | LLR Mean | Initial ncheck | Outcome |
|--------|----------|----------------|---------|
| **W1FC** (success) | 2.67 | 0/83 (0%) | ✅ Converged iter 0 |
| **K1BZM** (fail) | 2.38 | 38/83 (46%) | ❌ Stuck at 10/83 |

**Insight**: Even with good LLR quality, if initial hard decisions have many wrong bits (leading to 38/83 failing parity checks), BP cannot converge.

### 2. BP Convergence is Binary

- Either converges immediately (0-16 iterations)
- Or gets stuck (no amount of iterations helps)
- K1BZM makes excellent early progress (38→10 failing checks) but still fails

### 3. LLR Implementation is Correct

We verified:
- ✅ LLR computation: `max_mag_1 - max_mag_0` (matches WSJT-X)
- ✅ Normalization: divide by std dev (matches WSJT-X)
- ✅ Scaling: multiply by 2.83 (matches WSJT-X scalefac)
- ✅ Gray code mapping: verified against WSJT-X source

### 4. Multi-Symbol Combining Degradation

**Critical observation from logs** (signal at 2191.8 Hz):
```
nsym=1: mean_abs_LLR=2.06, max=14.09
nsym=2: mean_abs_LLR=0.43, max=21.75  (5.5x degradation!)
```

**Evidence of phase/combining issues**:
- LLR mean drops dramatically with nsym=2/3
- BUT: Disabling nsym=2/3 made things worse (11 → 9 decodes)
- Some signals NEED multi-symbol combining despite degradation

---

## Hypotheses

### Primary Hypothesis: Symbol Extraction Quality

**Evidence**:
1. Initial hard decisions have too many errors (38/83 parity failures)
2. Multi-symbol combining degrades LLRs dramatically
3. Strong signals fail while some weak signals succeed (counter-intuitive)

**Possible causes**:
1. **Phase tracking errors** in multi-symbol combining
2. **Residual frequency offset** causing phase drift
3. **Gray code soft decoding** has subtle bugs
4. **Symbol FFT timing** slightly off

### Secondary Hypothesis: WSJT-X Multi-Pass Strategy

**What we're missing**:
- WSJT-X tries 4 different LLR variants (bmeta, bmetb, bmetc, bmetd)
- Each uses different nsym values (1, 2, 3)
- They may apply additional normalization or weighting
- Our single-pass approach might miss signals that need specific parameters

### Tertiary Hypothesis: A-Priori Information

**What we haven't explored**:
- WSJT-X uses a-priori information (apmask, mcq arrays) to bias certain bits
- They apply contest-specific message biases
- We're doing pure blind decoding with no priors

---

## Test Results

### OSD Order Experiment

**Configuration**:
- Increased OSD order: 2 → 4
- Tested on 210703_133430.wav

**Results**:
- Before: 11/22 messages (50%)
- After:  11/22 messages (50%)
- **No improvement**

**Conclusion**: OSD is not the limiting factor. The LLR quality is insufficient for both BP and OSD to succeed.

---

## Diagnostic Output Analysis

From 65,951 lines of LDPC output:

**BP Failure Distribution**:
```
588 stuck at ncheck=39/83  (47% failing)
494 stuck at ncheck=44/83  (53% failing)
462 stuck at ncheck=38/83  (46% failing)
446 stuck at ncheck=43/83  (52% failing)
...
```

**BP Success Pattern**:
```
Most converge at iteration 0-2
Slowest success: iteration 16
Average: ~3-4 iterations for successes
```

**Key observation**: Success/failure is determined in the first few iterations. Extended iterations don't help once stuck.

---

## Recommended Next Steps

### Priority 1: Debug Multi-Symbol Combining 🔴

**Task**: Investigate why nsym=2/3 degrades LLR quality by 5.5x

**Approach**:
1. Add detailed logging to multi-symbol combining code
2. Check phase coherence between symbols
3. Verify complex amplitude summation is correct
4. Compare with WSJT-X's coherent combining implementation

**Files**: `src/sync/extract.rs` lines 421-470

### Priority 2: Verify Symbol Extraction Timing 🟡

**Task**: Ensure symbol FFT windows are correctly aligned

**Approach**:
1. Verify `nsps_down` calculation matches WSJT-X
2. Check FFT window placement (currently no offset)
3. Test with different window offsets
4. Verify symbol boundaries are correct

**Files**: `src/sync/extract.rs` lines 220-290

### Priority 3: Implement WSJT-X Multi-Pass Strategy 🟢

**Task**: Try multiple LLR variants like WSJT-X

**Approach**:
1. Implement normalized LLR variant: `bm / max(max_mag_1, max_mag_0)`
2. Try different nsym for different decode passes
3. Save best result across all attempts

**Files**: `src/decoder.rs` decode loop

### Priority 4: Add A-Priori Information (Future) ⚪

**Task**: Implement WSJT-X's a-priori message biasing

**Approach**:
1. Study `wsjtx/lib/ft8/ft8b.f90` apmask logic
2. Implement CQ and standard exchange biasing
3. Apply to top candidates only (to limit false positives)

**Note**: This is lower priority as it's an optimization, not a fix

---

## Files Modified in This Session

1. **src/ldpc/decode.rs**:
   - Added (then commented out) detailed BP iteration logging
   - Lines 86-89, 96, 99, 105: Diagnostic eprintln statements

2. **src/ldpc/mod.rs**:
   - Increased OSD order: 2 → 4 (line 64)
   - No decode rate improvement observed

3. **src/decoder.rs**:
   - Added (then commented out) LDPC attempt logging
   - Lines 143-144: LDPC_ATTEMPT diagnostic

4. **docs/ldpc_convergence_analysis.md**:
   - Comprehensive analysis of BP convergence patterns
   - Documented findings and hypotheses

5. **docs/investigation_20251122_continued.md**:
   - This file - continuation summary

---

## Statistics

**Test Duration**: ~5 seconds per run with full diagnostics
**Diagnostic Output**: 65,951 lines (6.5MB)
**LDPC Attempts**: Thousands (varies with scaling factors and nsym)
**Successful BP Convergences**: ~16-20 (leading to 11 unique messages)
**OSD Successes**: 2 (seen in logs with "OSD succeeded" messages)

---

## Conclusions

1. **LDPC implementation is correct**: Algorithm matches WSJT-X, normalization is correct
2. **OSD is not the bottleneck**: Increasing order doesn't help
3. **Problem is LLR quality**: Initial hard decisions have too many errors
4. **Multi-symbol combining is suspect**: Severe LLR degradation observed
5. **Symbol extraction needs investigation**: Root cause likely in phase tracking or timing

**Next session should focus on**: Debugging multi-symbol combining and verifying symbol extraction timing.

---

## Quick Test Command

```bash
# Run test with diagnostic output
cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored --nocapture 2>&1 | tee test_output.txt

# Extract decode summary
tail -50 test_output.txt

# Compare with WSJT-X
wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 -d 3 tests/test_data/210703_133430.wav
```
