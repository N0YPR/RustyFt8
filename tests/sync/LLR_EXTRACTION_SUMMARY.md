# LLR Extraction Summary

## What Was Done

Successfully extracted **pre-decode** LLR (Log-Likelihood Ratio) values from WSJT-X's FT8 decoder for the message "N1PJT HB9CQK -10" and created a unit test to benchmark RustyFt8's LDPC decoder performance.

## Key Achievement

✅ **Proper Pre-Decode Extraction**: LLR values are captured BEFORE LDPC error correction, right after symbol demodulation. This makes the test a true benchmark rather than a circular test.

## Files Created/Modified

### 1. Modified WSJT-X Decoder
**File**: `tests/sync/ft8b_llr_extract.f90`

Added code at line 243-265 to extract LLR values before LDPC:
```fortran
! Output LLR values BEFORE LDPC decoding for target frequency
if(abs(f1-466.0).lt.5.0) then
   write(*,*) 'LLR values (llra - 174 elements):'
   ! ... outputs LLRs before decode174_91 is called
endif
```

### 2. Unit Test in LDPC Module
**File**: `src/ldpc/decode.rs`

Added test function `test_decode_real_wsjt_x_llr_n1pjt_hb9cqk` (lines 414-510):
- Contains 174 real pre-decode LLR values from WSJT-X
- Marked with `#[ignore]` because it currently **fails**
- Serves as a benchmark for LDPC improvement

### 3. Documentation
**File**: `tests/LLR_EXTRACTION_README.md`

Complete documentation of the extraction process, build steps, and usage.

## Test Results

### Current Status: ❌ **FAILING** (Expected)

```
=== Testing LDPC Decode for Real WSJT-X Signal ===
Message: N1PJT HB9CQK -10
SNR: -10 dB
LLR stats: mean=2.44, max=6.26

LDPC decode failed for real WSJT-X signal.
WSJT-X successfully decoded this -10 dB message with 20 hard errors.
```

### Why This Is Good

The test **should fail** at this stage because:
1. LLR values represent real noisy signal (-10 dB SNR)
2. WSJT-X successfully decodes them (proving they're valid)
3. RustyFt8 currently cannot (revealing performance gap)

### How to Run

```bash
# Run the ignored test
cargo test --lib ldpc::decode::tests::test_decode_real_wsjt_x_llr_n1pjt_hb9cqk -- --ignored --nocapture
```

## Signal Details

- **Message**: N1PJT HB9CQK -10
- **Frequency**: 465.625 Hz
- **Time offset**: 0.75 seconds
- **SNR**: -10 dB
- **Sync power**: 1.79e+09
- **Source**: `tests/test_data/210703_133430.wav`

## Next Steps

When improving the LDPC decoder:

1. **Run this test** to check if improvements work on real signals
2. **Remove `#[ignore]`** once the test passes
3. **Add more tests** for different SNR levels using the same extraction method

## Extraction Method Reusability

The modified `ft8b_llr_extract.f90` can be easily adapted to extract LLRs for:
- Different messages (change frequency filter)
- Different SNR levels (other signals in the same or different WAV files)
- Messages decoded with AP (a priori) decoding

Simply adjust the frequency filter on line 246:
```fortran
if(abs(f1-466.0).lt.5.0) then  ! Change target frequency here
```

## Build Notes

To rebuild WSJT-X with modifications:
1. Modify `ft8b.f90` in source
2. Repack `wsjtx.tgz`: `tar -czf wsjtx.tgz wsjtx/`
3. Update MD5: `md5sum wsjtx.tgz > wsjtx.tgz.md5sum`
4. Clean rebuild: `rm -rf build/wsjtx-prefix && make -j32`

Dependencies required:
- cmake, gfortran
- libboost-all-dev
- qtbase5-dev, qttools5-dev, qtmultimedia5-dev
- libfftw3-dev, libusb-1.0-0-dev, libudev-dev
