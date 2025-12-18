# N1PJT HB9CQK -10 Decode Investigation

## Summary

**SOLVED!** This document summarizes the investigation into why the N1PJT HB9CQK -10 signal from the test file `210703_133430.wav` could not initially be decoded by RustyFt8's BP decoder, while WSJT-X successfully decodes it.

**Final Status**: N1PJT HB9CQK -10 is now successfully decoded using the hybrid BP/OSD decoder with accumulated LLR snapshots.

## Solution

The key insight came from analyzing WSJT-X's `decode174_91.f90`:

```fortran
zsum=zsum+zn                    ! WSJT-X accumulates LLRs across iterations
if(iter.gt.0 .and. iter.le.maxosd) then
   zsave(:,iter)=zsum           ! Saves ACCUMULATED sum, not per-iteration values
endif
```

WSJT-X **accumulates** LLRs across BP iterations (`zsum = zsum + zn`) and uses these accumulated values for OSD fallback. This smoothing effect significantly improves OSD performance on difficult signals.

### Changes Made

1. **Accumulated LLR Snapshots** (`src/ldpc/decode.rs`):
   - Modified `decode_with_snapshots()` to accumulate LLRs like WSJT-X
   - Saves `zsum` (accumulated) instead of `zn` (per-iteration) at iterations 1, 2, 3

2. **Hybrid Decoder Update** (`src/ldpc/mod.rs`):
   - Uses OSD with BP-accumulated snapshots instead of channel LLRs
   - Tries ndeep=3 then ndeep=4 for progressive depth
   - Increased parity check threshold from 36 to 50 (parity violations != bit errors)

3. **Test Results**:
   ```
   === Testing decode_hybrid directly ===
   decode_hybrid returned Some!
     iters=0, nharderrors=41
     Message: "N1PJT HB9CQK -10"

   *** N1PJT DECODED via decode_hybrid! ***
   ```

## Key Findings

### 1. LLR Sign Convention is Correct

**Convention B is used throughout:**
- **Extraction** (`sync/extract.rs:526`): `llr = max_mag_1 - max_mag_0`
  - Positive LLR = bit 1 more likely
  - Negative LLR = bit 0 more likely

- **LDPC Decoder** (`ldpc/decode.rs`): Expects positive LLR = bit is 1

Both components use Convention B consistently. The sign convention was NOT the problem.

### 2. Minimum Achievable Hard Errors

After extensive grid search across frequency and time offsets:

| Parameter | WSJT-X | Best RustyFt8 |
|-----------|--------|---------------|
| Frequency | 465.62 Hz | 464.04 Hz |
| Time offset | 0.75s | 0.710s |
| Hard errors | 20 (after decode) | 16-19 (before decode) |

### 3. BP Decoder Capacity

Testing with synthetic error injection:
- **10 errors**: BP successfully corrects
- **16-19 errors**: BP fails to converge

Our BP decoder can correct approximately **10-15 errors**. For harder signals (16-20 errors), OSD fallback with accumulated LLRs is required.

### 4. Accumulated LLR Improvement

| LLR Source | Hard Errors | OSD ndeep=3 | OSD ndeep=4 |
|------------|-------------|-------------|-------------|
| Channel LLRs | 19 | Failed | Failed |
| Accumulated iter 1 | 16 | Failed | **Success!** |
| Accumulated iter 2 | 14 | Failed | **Success!** |
| Accumulated iter 3 | 13 | Failed | Failed |

The accumulated LLRs from BP iteration 1-2 have fewer hard errors and better structure for OSD decoding.

## Expected Codeword

For message "N1PJT HB9CQK -10" (from `ft8code`):

```
Info bits (77):  00001010010100001100011011110100000011101011101101011111000111111010101001001
CRC (14):        10110111001101
Parity (83):     10110000100001110000010111100001110101100111101100010011111000010010111101111001010
```

## BP Improvements Implemented

1. **Damping** (`decode_damped`): Blends old and new messages to prevent oscillations
   - Tested damping factors: 0.1, 0.2, 0.25, 0.3, 0.4, 0.5
   - Helps some signals but not enough for N1PJT alone

2. **Early Stopping** (matching WSJT-X): Exits early if stuck for 5 iterations after iter 10 with >15 errors
   - Improves efficiency

3. **LLR Scaling**: Tries different LLR magnitudes (0.5, 0.75, 1.5, 2.0)
   - Sometimes helps marginal signals

4. **Accumulated LLR Snapshots**: The key breakthrough!
   - Saves accumulated LLRs at iterations 1, 2, 3
   - OSD uses these instead of raw channel LLRs

5. **Hybrid Decoder**: Combines all techniques:
   - BP with snapshots → damped BP → scaled BP → OSD on accumulated snapshots

## Test Files

- `examples/test_improved_bp.rs` - Tests all BP/OSD improvements on N1PJT
- `examples/test_accumulated_llr.rs` - Compares accumulated LLRs with WSJT-X
- `examples/test_hybrid_direct.rs` - Direct test of decode_hybrid
- `examples/debug_hybrid.rs` - Debug tracing for hybrid decoder
