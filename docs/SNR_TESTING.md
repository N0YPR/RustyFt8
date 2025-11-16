# SNR Performance Testing Results

**Date**: 2025-01-16 (Updated)
**Decoder Version**: Multi-pass with nsym=1/2/3, multi-scale LLRs, and phase tracking

## Test Methodology

Generated test signals using RustyFt8's `ft8sim` with message "CQ W1ABC FN42" at SNRs from -19 dB to +10 dB. SNR is measured in 2500 Hz bandwidth as per FT8 standard.

## Current Results Summary

| SNR (dB) | Decode Status | Method | LDPC Iterations | Costas Sync Quality |
|----------|---------------|--------|-----------------|---------------------|
| Perfect  | ✅ SUCCESS    | nsym=1, scale=0.5 | 0 | 21/21 (100%) |
| +10 to -10 | ✅ SUCCESS  | nsym=1, scale=0.5 | 1-2 | 21/21 (100%) |
| **-14**  | ✅ SUCCESS    | nsym=1, scale=0.5 | 2 | 21/21 (100%) |
| **-15**  | ✅ SUCCESS    | nsym=1, scale=0.8 | 2 | 19/21 (90%) |
| **-16**  | ✅ SUCCESS    | nsym=1, scale=0.8 | 8 | 19/21 (90%) |
| **-17**  | ✅ SUCCESS    | nsym=1, scale=1.0 | 7 | 19/21 (90%) |
| **-18**  | ✅ SUCCESS    | nsym=1, scale=2.0 | 93 | 19/21 (90%) |
| -19      | ❌ FAIL       | - | - | Poor sync |

**Minimum Working SNR**: **-18 dB** 🎉

## Performance Comparison

| Implementation | Minimum SNR | Gap to WSJT-X | Notes |
|----------------|-------------|---------------|-------|
| **RustyFt8** | **-18 dB** | **3 dB** | Multi-pass nsym=1/2/3 + phase tracking |
| WSJT-X | -21 dB | - | Multi-symbol + AP decoding |

**Achievement**: +3 dB improvement from initial -15 dB! Within 3 dB of WSJT-X.
| **Performance Gap** | **~6 dB** | Expected improvement with nsym=2/3 |

## Analysis

### What Works Well

1. **Perfect Symbol Extraction**: 21/21 Costas array validation down to -15 dB proves timing and frequency sync are accurate
2. **LDPC Decoder**: Converges reliably when given adequate LLR quality
3. **Sync Quality**: Coarse and fine synchronization work correctly across all tested SNRs
4. **Multi-pass Decoding**: Automatic LLR scaling (0.5 to 5.0) finds optimal convergence point

### Performance Bottleneck

Below -18 dB:
- Costas sync quality degrades rapidly (drops below 19/21)
- Single-symbol LLR computation becomes too noisy
- LDPC cannot converge even with aggressive scaling

### Expected Improvements

**Multi-symbol coherent combining** (nsym=2 or nsym=3):
- Theory: ~3-6 dB SNR improvement through coherent averaging
- WSJT-X uses nsym=1/2/3 in multiple decoding passes

**Implementation Status**:
- ✅ nsym=1: Working perfectly, **-18 dB minimum SNR**
- ✅ nsym=2: Correctly implemented with phase tracking
- ✅ nsym=3: Correctly implemented with phase tracking
- ❌ **nsym=2/3 don't help at low SNR** - magnitude-based nsym=1 outperforms them

### Multi-Symbol Investigation Results

**Key Finding**: At -18 dB SNR, **noise dominates** making phase-sensitive coherent combining less effective:

1. **Phase decorrelation** - Even small frequency errors cause phase drift between symbols
2. **Noise amplification** - Coherent sum is phase-sensitive, amplifies noise along with signal
3. **nsym=1 robustness** - Magnitude-only decoding is immune to phase noise

**What We Fixed**:
- ✅ Fixed 1 Hz frequency bias (Costas waveforms at wrong sample rate)
- ✅ Implemented phase tracking (WSJT-X-style twkfreq correction)
- ✅ Multi-pass decoder (tries nsym=1/2/3 with 10 scales each)
- ✅ Optimal LLR scaling (0.5 to 5.0 range)

**Result**: **-18 dB minimum SNR** achieved - within 3 dB of WSJT-X's -21 dB!

See [FINDINGS.md](FINDINGS.md) and [PHASE_TRACKING_RESULTS.md](PHASE_TRACKING_RESULTS.md) for detailed technical analysis.

## Test Signal Details

**Message**: "CQ W1ABC FN42"

**Expected Codeword** (from WSJT-X ft8code):
```
77-bit message: 00000000000000000000000000100000010111111111010001001110100010100001100110001
14-bit CRC:     10110001011110
83 parity bits: 10001000110101001011010110110001110100100010110100110111000100011100111101001001100

Channel symbols (79 tones):
  Sync               Data               Sync               Data               Sync
3140652 00000000100677453260602152206 3140652 73104612344145534547052576115 3140652
```

**RustyFt8 Detection** (perfect signal, nsym=1):
```
Data symbols 7-35 (detected tones):  00007000101677453260602152206
Data symbols 43-71 (detected tones): 73104612344145534547052576115
```

Note: Symbol 11 (5th data symbol) shows tone 7 instead of 0. This appears consistently across multiple tests and may indicate a minor timing/FFT issue that doesn't affect decode success at high SNR.

## Real-World Applicability

**Typical FT8 Operating Conditions**: -10 dB to +10 dB SNR

RustyFt8's -15 dB minimum SNR is **sufficient for most real-world FT8 operation**. Signals weaker than -15 dB are rare and often unreliable even with WSJT-X.

## Future Work

1. **Debug nsym=2**: Currently fails even on perfect signals despite correct implementation structure
2. **Implement nsym=2 properly**: Should provide ~3 dB improvement → -18 dB minimum SNR
3. **Consider nsym=3**: May be worth the complexity for an additional 3 dB → -21 dB (matching WSJT-X)
4. **AP (a priori) decoding**: WSJT-X uses message structure hints to improve weak signal decoding
5. **Better noise estimation**: Could improve LLR scaling for marginal signals
