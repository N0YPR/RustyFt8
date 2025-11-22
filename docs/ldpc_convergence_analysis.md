# LDPC Convergence Analysis

**Date**: 2025-11-22
**Current Status**: 11/22 messages decoded (50%)
**Bottleneck**: LDPC BP convergence failure

---

## Key Findings

### 1. BP Convergence Patterns

**Successful Decodes** (11 messages):
- Converge VERY quickly: 0-16 iterations (mostly 0-2)
- Initial parity check state: 0-32 failing checks
- Example: W1FC at 2572.7 Hz converged at iter 0 with ncheck=0/83 (perfect!)
- **Pattern**: When BP works, it works immediately

**Failed Decodes** (11 messages):
- BP gets stuck after initial progress
- Distribution of stuck ncheck values:
  - Most common: 39/83, 44/83, 38/83, 43/83 (45-53% failing)
  - Best case: 10/83 (K1BZM) - still fails despite only 12% failing!
- **Pattern**: BP makes early progress then plateaus

---

## 2. K1BZM Deep Dive (2695.3 Hz, -3 dB SNR)

**Pipeline Status**:
- ✅ Found by coarse sync at rank 19
- ✅ Fine sync: 2695.3 Hz, dt=-0.12s, sync=0.852
- ✅ Extraction (nsym=1): Costas 20/21, LLR mean=2.38, max=6.97
- ❌ LDPC: Fails to converge

**BP Convergence Detail**:
```
nsym=1, scale=1.0:
  iter 0:  38/83 failing (46%)
  iter 10: 13/83 failing (16%)
  iter 20: 10/83 failing (12%)
  iter 50: 10/83 failing (12%)  ← STUCK!

nsym=1, scale=1.5:
  iter 0:  38/83 failing
  iter 10: 10/83 failing
  iter 20:  6/83 failing (7%)
  iter 50:  9/83 failing  ← Oscillates!
```

**Analysis**:
- BP makes excellent progress (38 → 10 failing checks)
- Gets stuck at last 6-10 checks (~7-12% failing)
- Increasing LLR scale helps initially but causes oscillation
- This suggests BP is trapped in a local minimum

---

## 3. Critical Insight: Initial Parity Check State

**Comparison**:

| Signal | LLR Mean | Initial ncheck | Outcome |
|--------|----------|----------------|---------|
| **W1FC** (success) | 2.67 | 0/83 (0%) | ✅ Converged iter 0 |
| **K1BZM** (fail) | 2.38 | 38/83 (46%) | ❌ Stuck at 10/83 |

**Key Finding**: Initial hard decisions from LLRs must be VERY accurate for BP to converge.

Even with good LLR quality (mean=2.38), if the initial parity check state has many failures (38/83), BP struggles to escape.

---

## 4. Multi-Symbol Combining Degradation

**LLR Quality vs nsym**:

| nsym | mean_abs_LLR | Quality |
|------|--------------|---------|
| 1 | 2.38 | ✅ Good |
| 2 | 0.43 | ❌ 5.5x worse! |
| 3 | Unknown | Likely degraded |

**Evidence from logs** (2191.8 Hz):
```
nsym=1: mean_abs_LLR=2.06, ncheck=38/83 → stuck at 27/83
nsym=2: mean_abs_LLR=0.43, ncheck=42/83 → stuck at 42/83 (NO progress!)
```

**Finding**: nsym=2/3 severely degrades LLR quality, making BP convergence even harder.

However, disabling nsym=2/3 made things WORSE (11 → 9 decodes), meaning some signals REQUIRE multi-symbol combining despite the degradation.

---

## 5. WSJT-X Comparison

**Algorithm Similarity**:
- WSJT-X uses same sum-product algorithm
- Same message passing equations
- Same tanh/atanh operations

**Key Difference - Early Stopping**:
```fortran
! WSJT-X: bpdecode174_91.f90, lines 69-84
if( ncnt .ge. 5 .and. iter .ge. 10 .and. ncheck .gt. 15) then
  return  ! Give up if no progress for 5 iterations
endif
```

**Analysis**:
- WSJT-X gives up early when BP stalls
- This is EFFICIENCY, not a solution
- It doesn't explain why WSJT-X succeeds where we fail
- The real question: Why do our LLRs lead to bad initial parity states?

---

## 6. Failed Decode Statistics

From 65,951 lines of LDPC diagnostic output:

**BP Convergence Distribution**:
```
Successful: 11 messages (16-20 total BP successes with different scales)
  - 0 iterations: 2 cases
  - 1-2 iterations: ~12 cases
  - 16 iterations: 1 case (slowest success)

Failed: Hundreds of attempts
  - 588 stuck at ncheck=39/83
  - 494 stuck at ncheck=44/83
  - 462 stuck at ncheck=38/83
  - 446 stuck at ncheck=43/83
  - [distribution continues...]
```

**Average stuck state**: ~40% of parity checks still failing

---

## 7. Root Causes (Hypotheses)

### Primary Hypothesis: LLR Calibration Issue

**Evidence**:
1. Good LLR quality (mean ~2-3) doesn't guarantee convergence
2. Initial hard decisions have too many errors (38/83 failing)
3. Multi-symbol combining destroys LLR quality (5.5x degradation)

**Possible Causes**:
1. **LLR scaling mismatch**: Our LLRs might not match the magnitude expected by BP
2. **Symbol extraction errors**: Phase coherence issues in multi-symbol combining
3. **Noise floor estimation**: Incorrect noise baseline leading to over/under-confident LLRs
4. **Gray code issues**: Errors in soft symbol to LLR conversion

### Secondary Hypothesis: Phase Coherence

**Evidence**:
1. Strong signals (-2 to -3 dB) failing while weak signals succeed
2. Multi-symbol combining degrades quality dramatically
3. Costas sync is excellent (20/21) but decoding fails

**Possible Causes**:
1. Phase tracking errors accumulate over symbols
2. Frequency offset residuals cause phase rotation
3. Multi-symbol combining amplifies phase errors

---

## 8. Investigation Next Steps

### Priority 1: Fix LLR Calibration 🔴

**Task 1.1**: Compare LLR distributions in detail
- Extract LLRs for K1BZM from our decoder
- Extract LLRs for same signal using WSJT-X (if possible)
- Compare bit-by-bit LLR values
- Identify systematic differences

**Task 1.2**: Investigate LLR normalization
- Study WSJT-X's symbol extraction and LLR computation
- Check if they apply additional normalization/scaling
- Test different LLR scaling approaches systematically

**Task 1.3**: Fix multi-symbol combining degradation
- Debug why nsym=2/3 degrades LLR quality by 5.5x
- Check phase coherence in `src/sync/extract.rs`
- Verify Gray code mapping for nsym=2/3
- Compare with WSJT-X multi-symbol logic

### Priority 2: Test BP Parameter Variations 🟡

**Task 2.1**: Early stopping criterion
- Implement WSJT-X's early stopping logic
- Test if it improves efficiency without changing results

**Task 2.2**: Damping factor
- Test BP with damping: `tov_new = alpha * tov_new + (1-alpha) * tov_old`
- Values to try: alpha = 0.5, 0.7, 0.9
- May help escape local minima

**Task 2.3**: Min-sum approximation
- Test min-sum algorithm instead of sum-product
- May be more robust to LLR calibration issues

### Priority 3: OSD Tuning 🟢

**Task 3.1**: Higher OSD order
- Current: order 2
- Test: order 3, 4, 5
- May recover signals where BP gets close (6-10 failing checks)

**Task 3.2**: OSD on partially converged states
- Save LLRs when BP gets stuck at 10/83 (like K1BZM)
- Try OSD on these "almost converged" states
- May be more effective than OSD on channel LLRs

---

## 9. Test Commands

```bash
# Run with full LDPC diagnostics
cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored --nocapture 2>&1 | tee ldpc_output.txt

# Extract K1BZM attempts
grep -A 10 "LDPC_ATTEMPT: freq=2695.3 Hz" ldpc_output.txt

# Compare with WSJT-X
wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 -d 3 tests/test_data/210703_133430.wav

# Analyze BP convergence patterns
grep "BP CONVERGED" ldpc_output.txt
grep "BP FAILED" ldpc_output.txt | awk '{print $NF}' | sort | uniq -c | sort -rn
```

---

## 10. Success Metrics

To consider LDPC convergence "fixed":

1. ✅ Decode K1BZM (currently gets to 10/83, needs to get to 0/83)
2. ✅ Decode at least 18/22 messages (82%+)
3. ✅ Decode ALL strong signals (-2 to -3 dB)
4. ✅ Multi-symbol combining doesn't degrade LLR quality

---

## 11. Key Insights

1. **BP convergence is binary**: Either converges in 0-16 iterations or gets stuck
2. **Initial state is critical**: Need 0-32 failing checks initially to converge
3. **Local minima are common**: BP gets stuck at 6-10 failing checks frequently
4. **LLR quality ≠ BP convergence**: Good mean LLR doesn't guarantee convergence
5. **Multi-symbol combining is broken**: Degrades LLRs by 5.5x but some signals need it

---

## References

- `SESSION_SUMMARY.md`: Complete session summary and investigation history
- `NEXT_STEPS.md`: Original investigation roadmap
- `decode_investigation_findings.md`: Pipeline analysis
- `ldpc_full_output.txt`: 65k lines of LDPC diagnostic output
- WSJT-X source: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/bpdecode174_91.f90`
