# LLR Value Extraction from WSJT-X

This document explains how LLR (Log-Likelihood Ratio) values were extracted from WSJT-X's FT8 decoder and used to create unit tests for RustyFt8.

## Overview

LLR values represent the "soft decision" information from the FT8 demodulator - they indicate not just whether a bit is 0 or 1, but also how confident the decoder is about that decision. These values are critical input to the LDPC error correction decoder.

## Extraction Process

### 1. Modified WSJT-X Decoder

The modified Fortran source code is located at: `tests/sync/ft8b_llr_extract.f90`

**Modifications made to WSJT-X's `ft8b.f90`:**
- Added code to detect when the message "N1PJT HB9CQK -10" is decoded
- Output the 174 LLR values used for LDPC decoding
- Output the AP (a priori) mask showing which bits had AP applied
- Output the final decoded message bits

**Key code addition (lines 433-469):**
```fortran
! ===== LLR EXTRACTION CODE =====
if(index(msg37,'N1PJT').gt.0 .and. index(msg37,'HB9CQK').gt.0) then
   write(*,*) 'LLR values (174 elements):'
   write(*,'(A)',advance='no') 'llrz=['
   do i=1,173
      write(*,'(F8.3,",")',advance='no') llrz(i)
   enddo
   write(*,'(F8.3,"]")') llrz(174)
   ! ... (also outputs AP mask and message bits)
endif
```

### 2. Building WSJT-X

Built WSJT-X 2.7.0 with the modifications:

```bash
cd /workspaces/RustyFt8/wsjtx/wsjtx-2.7.0
mkdir -p build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release -DWSJT_GENERATE_DOCS=OFF -DWSJT_SKIP_MANPAGES=ON
make -j32
```

Dependencies installed:
- `cmake`
- `gfortran`
- `libboost-all-dev`
- `qtbase5-dev`, `qttools5-dev`, `qtmultimedia5-dev`
- `libfftw3-dev`
- `libusb-1.0-0-dev`

### 3. Running the Decoder

Executed the modified `jt9` on the real FT8 recording:

```bash
/workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 \
    -8 -p 15 /workspaces/RustyFt8/tests/test_data/210703_133430.wav
```

**Output captured:**
```
===== LLR VALUES FOR TARGET MESSAGE =====
Message: N1PJT HB9CQK -10
Pass: 4
iaptype: 0
Frequency: 465.625000 Hz
Time offset: 0.750000000 s
Sync: 1.79003302E+09
Hard errors: 20

LLR values (174 elements):
llrz=[-2.803, 1.876, -2.974, ... (174 values total)]

AP mask (174 elements):
apmask=[0,0,0,0,0, ... (all zeros - no AP used)]

Message bits (77 elements):
message77=[0,0,0,0,1,0,1,0,0,1, ... (77 bits)]
```

## Test Implementation

### Unit Test: `test_decode_real_wsjt_x_llr_n1pjt_hb9cqk`

Location: `src/ldpc/decode.rs` (in the `#[cfg(test)]` module)

**Purpose:** Verify that RustyFt8's LDPC decoder can decode the same message that WSJT-X decoded from real FT8 signals.

**Current Status:** ❌ **FAILING** (as expected)

```
=== Testing LDPC Decode for Real WSJT-X Signal ===
Message: N1PJT HB9CQK -10
SNR: -10 dB
LLR stats: mean=2.44, max=6.26

LDPC decode failed for real WSJT-X signal.
WSJT-X successfully decoded this -10 dB message with 20 hard errors.
This indicates the LDPC decoder needs improvement.
```

**Why it fails:** The LLR values are extracted BEFORE LDPC error correction (pre-decode), so they represent the actual noisy signal. WSJT-X successfully decodes these, but RustyFt8's LDPC decoder currently cannot.

**Goal:** This test serves as a benchmark. When RustyFt8 can pass this test, it means the LDPC decoder has achieved WSJT-X's level of performance for -10 dB SNR signals.

## Signal Parameters

**Source:**
- WAV file: `tests/test_data/210703_133430.wav`
- Real FT8 recording from 2021-07-03 13:34:30 UTC

**Decode Parameters:**
- **Message**: N1PJT HB9CQK -10
- **Frequency**: 465.625 Hz
- **Time offset**: 0.75 seconds
- **SNR**: -10 dB
- **Sync power**: 1.79e+09
- **Decoder pass**: 4 (nsym=1, bit-by-bit normalized)
- **AP type**: 0 (no a priori decoding used)

## LLR Statistics

**For the -10 dB SNR signal:**
- Mean |LLR|: 2.667
- Max |LLR|: 4.017
- Min |LLR|: 0.220

These values are reasonable for a moderately weak signal. Stronger signals would have higher magnitude LLRs (more confident decisions), while weaker signals would have lower magnitudes.

## Usage

To run the test (it's currently marked `#[ignore]` because it fails):

```bash
cargo test --lib ldpc::decode::tests::test_decode_real_wsjt_x_llr_n1pjt_hb9cqk -- --ignored --nocapture
```

Or run all LDPC tests including ignored ones:

```bash
cargo test --lib ldpc::decode::tests -- --ignored --nocapture
```

Once the LDPC decoder is improved, remove the `#[ignore]` attribute from the test.

## Future Improvements

This extraction method can be used to create additional test cases:
1. Extract LLRs for messages at different SNR levels (-24 dB to +16 dB)
2. Extract LLRs for messages decoded with AP (a priori) decoding
3. Extract LLRs for messages requiring OSD (Ordered Statistics Decoding)
4. Compare RustyFt8 vs WSJT-X performance across SNR ranges

## References

- **WSJT-X Source**: https://sourceforge.net/projects/wsjt/files/wsjtx-2.7.0/
- **FT8 Protocol**: https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
- **Modified Fortran Source**: `tests/sync/ft8b_llr_extract.f90`
- **Unit Test**: `tests/test_llr_n1pjt_hb9cqk.rs`
