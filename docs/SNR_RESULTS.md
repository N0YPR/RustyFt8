# SNR Performance Test Results

## Multi-Pass Decoder Performance

Tested with message: "CQ W1ABC FN42"
Test date: Post multi-pass implementation

| SNR (dB) | Result | Pass | nsym | Scale | LDPC Iters | Notes |
|----------|--------|------|------|-------|------------|-------|
| -14      | ✅ PASS | 1    | 1    | 0.5   | 2          | Instant decode |
| -15      | ✅ PASS | 2    | 1    | 0.8   | 2          | Quick decode |
| -16      | ✅ PASS | 2    | 1    | 0.8   | 8          | Moderate iterations |
| -17      | ✅ PASS | 3    | 1    | 1.0   | 7          | Increasing difficulty |
| **-18**  | **✅ PASS** | **6** | **1** | **2.0** | **93** | **Minimum SNR achieved** |
| -19      | ❌ FAIL | -    | -    | -     | -          | Below threshold |

## Performance Summary

- **Minimum Working SNR**: **-18 dB**
- **WSJT-X Reference**: -21 dB
- **Performance Gap**: 3 dB (excellent!)
- **Previous Achievement**: -15 dB (single-pass)
- **Improvement**: +3 dB SNR gain from multi-pass strategy

## Key Insights

1. **All successful decodes used nsym=1** - The improvement came entirely from trying multiple LLR scaling factors, not from nsym=2/3 coherent combining

2. **Scaling factors are crucial** - Different noise conditions require different LLR scales:
   - High SNR (-14 to -15 dB): scale=0.5-0.8
   - Medium SNR (-16 to -17 dB): scale=0.8-1.0  
   - Low SNR (-18 dB): scale=2.0

3. **LDPC iterations scale with SNR** - From 2 iterations at -14 dB to 93 iterations at -18 dB

4. **nsym=2/3 not providing benefit yet** - Despite correct implementation, coherent combining across multiple symbols isn't outperforming single-symbol decoding with optimal scaling. This suggests minor frequency synchronization issues remain.

## Conclusion

The multi-pass strategy with multiple LLR scaling factors proved highly effective, bringing RustyFt8 to within 3 dB of WSJT-X's world-class -21 dB performance. This is sufficient for the vast majority of real-world FT8 operation.
