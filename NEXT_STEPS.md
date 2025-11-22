# Next Steps: LDPC Decoder Investigation

**Last Updated**: 2025-11-22 (After Pipeline Analysis)
**Current Status**: 11/22 messages decoded (50%)

## 🎯 Root Cause Identified: LDPC Convergence Failure

After extensive pipeline analysis, we've isolated the problem:

**The entire signal processing pipeline works correctly**, but **LDPC fails to decode** signals with excellent extraction quality.

---

## Investigation Summary

### ✅ What Works (Verified)
1. **Candidate Generation**: All expected signals found (150 candidates total)
2. **Fine Synchronization**: Correctly refines frequency within ±2.5 Hz
3. **Symbol Extraction**: Produces excellent Costas sync (13-20/21) and LLR quality (mean ~2.3-2.4)
4. **Pipeline Reaches LDPC**: All key signals attempt decoding with nsym=1,2,3

### ❌ What Fails
**LDPC Decoder** - Fails to converge despite:
- Costas sync: 20/21 for K1BZM (better than many successful decodes!)
- LLR quality: mean=2.38 (good)
- Correct frequency/timing
- Multiple nsym attempts

### Example: K1BZM EA3GP -09 (2695 Hz)
| Stage | Result | Quality |
|-------|--------|---------|
| Coarse sync | Found at 2695.3 Hz, rank 20 | ✅ Good |
| Fine sync | Refined to 2695.3 Hz, dt=-0.12s | ✅ Correct |
| Extraction (nsym=1) | Costas 20/21, LLR 2.38 | ✅ Excellent |
| Extraction (nsym=2) | Costas 20/21, LLR 0.43 | ⚠️ LLR degraded 5.5x! |
| LDPC | Never converges | ❌ Fails |

---

## 🔴 PRIORITY 1: Investigate LDPC Convergence

### Task 1.1: Add LDPC Diagnostic Logging

Add logging to `src/ldpc/mod.rs` to track:
- Number of iterations attempted
- Syndrome weight at each iteration
- Which bits are flipping
- Why it stops (converged vs max iters vs gave up)

```rust
// In LDPC decoder
eprintln!("LDPC_ATTEMPT: freq={:.1} Hz, nsym={}, initial_errors={}",
          freq, nsym, initial_syndrome_weight);

for iter in 0..max_iters {
    eprintln!("  Iter {}: syndrome_weight={}, flips={}",
              iter, syndrome_weight, num_flips);
    if converged {
        eprintln!("  CONVERGED at iteration {}", iter);
        break;
    }
}

if !converged {
    eprintln!("  FAILED: max_iters reached, final_syndrome_weight={}",
              syndrome_weight);
}
```

### Task 1.2: Compare LLR Distributions

Extract and compare LLR distributions:
- Successful decode (e.g., W1FC at 2572 Hz)
- Failed decode (e.g., K1BZM at 2695 Hz)

Check if failed signals have:
- Lower LLR magnitudes
- More uncertain bits (low magnitude)
- Different distribution shape

### Task 1.3: Test LDPC Parameter Variations

Try different LDPC parameters for the failing signals:
1. **More iterations**: Increase from current max to 50, 100, 200
2. **Higher OSD order**: Try order 3, 4, 5 instead of 2
3. **Different LLR scaling**: Test factors 1.0, 1.5, 2.0, 3.0
4. **Looser convergence**: Allow small syndrome weights to pass

### Task 1.4: Check Multi-Symbol LLR Quality

**Critical observation**: LLR quality DROPS with multi-symbol combining:
- nsym=1: mean_abs_LLR=2.38
- nsym=2: mean_abs_LLR=0.43 (5.5x worse!)

Investigate why multi-symbol combining degrades quality:
1. Check phase coherence in `extract.rs`
2. Verify Gray code mapping for nsym=2/3
3. Compare with WSJT-X multi-symbol logic
4. Test with nsym=1 only to see if it helps

---

## 🟡 PRIORITY 2: Compare WSJT-X LDPC Implementation

### Files to Study
```
wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/
├── bpdecode174_91.f90  # BP decoder
├── osd174_91.f90       # OSD decoder
└── normalizebmet.f90   # LLR normalization
```

### Key Questions
1. **LLR scaling**: Does WSJT-X apply different normalization?
2. **Iteration limits**: What max iterations do they use?
3. **Convergence criteria**: What syndrome weight threshold?
4. **OSD strategy**: When do they escalate to higher orders?
5. **Multi-pass logic**: Do they try different LLR scalings?

---

## 🟢 PRIORITY 3: Analyze Decoded vs Missing Signals

### Frequency Distribution
Check if we're missing specific frequency bands:
```bash
# Get WSJT-X frequencies
wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 -d 3 \
  tests/test_data/210703_133430.wav | awk '{print $5}' | sort -n

# Compare with our decodes
```

### SNR Distribution
Are we systematically missing a certain SNR range?

| SNR Range | Expected | Decoded | Gap |
|-----------|----------|---------|-----|
| Strong (-2 to -3 dB) | 3 | 0 | 100% missing! |
| Medium (-8 to -10 dB) | 8 | 6 | 25% missing |
| Weak (-14 to -23 dB) | 11 | 5 | 55% missing |

**Strong signals are failing more than weak signals!** This is counter-intuitive and suggests:
1. Strong signals might have different characteristics (sharper peaks, less averaging)
2. Our LDPC decoder might be over-confident on strong signals
3. Multi-symbol combining might hurt strong signals more

---

## Quick Commands

```bash
# Run test with full logging
cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored --nocapture 2>&1 | tee decode_log.txt

# Find LDPC attempts for specific frequency
grep -E "(LDPC|OSD)" decode_log.txt | grep -B 2 "2695"

# Compare LLR distributions
grep "mean_abs_LLR" decode_log.txt | sort -t'=' -k4 -n

# Check WSJT-X output
wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 -d 3 tests/test_data/210703_133430.wav
```

---

## Performance Tracking

| Milestone | Messages | Success Rate | Key Achievement |
|-----------|----------|--------------|-----------------|
| Baseline | 8/19 | 42% | Initial state |
| Time penalty | 10/19 | 53% | Candidate selection improved |
| Norm fix | 11/22 | 50% | **Fixed critical downsample bug** |
| Pipeline verified | 11/22 | 50% | **Isolated problem to LDPC** |
| **Target** | **22/22** | **100%** | **Fix LDPC convergence** |

---

## Test Data: Three Key Failing Signals

Focus investigation on these well-characterized failures:

### 1. K1BZM EA3GP -09 (2695 Hz)
- Expected: SNR=-3 dB, dt=-0.1s
- Candidate: rank 20, coarse_sync=4.976
- Fine sync: 2695.3 Hz, dt=-0.12s, sync=0.852
- Extraction: Costas 20/21, LLR 2.38 (nsym=1)
- **Status**: LDPC fails despite excellent quality

### 2. N1PJT HB9CQK -10 (466 Hz)
- Expected: SNR=-2 dB, dt=0.2s
- Candidate: rank ~35, coarse_sync=3.290 (via 468.8 Hz)
- Fine sync: 466.2 Hz, dt=0.21s, sync=0.984
- Extraction: Costas 13/21, LLR 2.36 (nsym=1)
- **Status**: LDPC fails despite good quality

### 3. CQ F5RXL IN94 (1197 Hz)
- Expected: SNR=-2 dB, dt=-0.8s
- Candidate: rank ~40, coarse_sync=3.215 (via 1195.3 Hz)
- Fine sync: 1196.8 Hz, dt=-0.76s, sync=1.161
- Extraction: Quality unknown (need to extract from interleaved logs)
- **Status**: LDPC fails

---

## Success Criteria

To consider the investigation complete:
1. ✅ **Understand** why LDPC fails on good signals
2. ✅ **Fix** LDPC to converge on at least 2/3 of the key failing signals
3. ✅ **Achieve** 18+/22 messages decoded (82%+)
4. ✅ **Decode** all strong signals (-2 to -3 dB)

---

## Documentation

- `docs/decode_investigation_findings.md`: Detailed analysis of pipeline
- `docs/decoder_analysis_real_recording.md`: Initial investigation
- `docs/investigation_20251122_summary.md`: Investigation summary
- `NEXT_STEPS.md`: This file (action plan)

---

Good luck fixing LDPC! 🚀

**Key Insight**: The problem isn't signal processing - it's error correction. We have excellent signals reaching LDPC that should decode easily, but the decoder can't recover the message bits. This is likely a parameter tuning issue or a subtle bug in the LDPC/OSD implementation.
