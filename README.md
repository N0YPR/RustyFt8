# RustyFt8
An implementation of FT8 using Rust.

## FT8 Message Transmission Chain

Based on the WSJT-X source code, here's the complete chain of steps for transmitting an FT8 message:

### 1. Message Parsing and Packing (77 bits)
- **Function**: `pack77()` in packjt77.f90
- Takes text message like "CQ W1ABC FN42"
- Determines message type (i3 field: 0-5)
- Determines subtype (n3 field for i3=0)
- Packs into exactly 77 bits of information

### 2. CRC Generation (14 bits)
- **Function**: `encode174_91()`
- Appends 3 zero bits to the 77-bit message (77 → 80 bits)
- Computes 14-bit CRC using `crc14()` function
- Produces 91-bit message (77 information + 14 CRC)

### 3. LDPC Forward Error Correction (83 parity bits)
- **Function**: `encode174_91()`
- Uses LDPC(174,91) code - a low-density parity check code
- Multiplies 91-bit message by generator matrix
- Produces 83 parity bits
- **Output**: 174-bit codeword (91 message + 83 parity)

### 4. Symbol Mapping (79 symbols, 0-7)
- **Function**: `genft8()`
- Takes 174 bits in groups of 3 → 58 data symbols (3 bits = 8-FSK symbol)
- Maps through Gray code: `graymap(0:7) = [0,1,3,2,5,6,4,7]`
- Interleaves with 3× Costas 7×7 sync patterns (21 symbols total)
- **Structure**: `S7 D29 S7 D29 S7` (7+29+7+29+7 = 79 symbols)
- Costas pattern: `[3,1,4,0,6,5,2]`
- **Output**: 79 tone numbers (0-7)

### 5. GFSK Pulse Shaping
- **Function**: `gen_ft8wave()`
- Uses Gaussian Frequency Shift Keying (GFSK) pulse shaping
- **Pulse function**: `gfsk_pulse()`
  - Based on error function (erf)
  - Bandwidth-time product (BT) parameter controls smoothness
  - Formula: `0.5 * (erf(c*b*(t+0.5)) - erf(c*b*(t-0.5)))`
- Creates smooth frequency transitions between tones
- Pulse spans 3 symbol periods (3 × 1920 = 5760 samples)

### 6. Frequency Modulation
- **Function**: `gen_ft8wave()`
- Each tone number (0-7) maps to frequency offset
- Tone spacing: 6.25 Hz (12000 Hz sample rate / 1920 samples per symbol)
- Smoothed frequency waveform created by convolving tones with GFSK pulse
- Base frequency f0 added to shift signal to desired RF frequency

### 7. Waveform Generation (12.64 seconds)
- **Function**: `gen_ft8wave()`
- **Parameters**:
  - 79 symbols × 1920 samples/symbol = 151,680 samples
  - At 12,000 samples/second = 12.64 seconds
- Generates sine wave: `sin(φ)` where φ accumulates based on frequency
- Can generate complex baseband (`cwave`) or real audio (`wave`)

### 8. Envelope Shaping
- **Function**: `gen_ft8wave()`
- Applies cosine-squared ramping to first and last symbols
- Ramp length: 1920/8 = 240 samples
- Prevents abrupt starts/stops that cause spectral splatter
- Formula: `(1 - cos(2πt/(2*nramp)))/2` for attack, similar for decay

### Summary Flow
```
Text Message (37 chars)
    ↓ pack77()
77-bit packed message
    ↓ encode174_91() - add CRC
91 bits (77 + 14 CRC)
    ↓ encode174_91() - LDPC encoding
174 bits (91 + 83 parity)
    ↓ genft8() - 3-bit grouping + Gray mapping
58 data symbols (0-7)
    ↓ genft8() - add sync (Costas arrays)
79 symbols: S7 D29 S7 D29 S7
    ↓ gen_ft8wave() - GFSK pulse shaping
Smooth frequency trajectory
    ↓ gen_ft8wave() - FM modulation
Phase-accumulated sine wave
    ↓ gen_ft8wave() - envelope shaping
151,680 audio samples (12.64 sec @ 12kHz)
    ↓
RF transmission
```

The key insight is that FT8 uses 8-FSK (8 frequency tones) with GFSK smoothing, strong LDPC error correction, and Costas sync patterns to achieve robust communication at very low signal-to-noise ratios (-21 dB).

## 📚 Key Documentation

**For AI Assistants & Developers**, please read:

- **[`AGENTS.md`](AGENTS.md)** - Comprehensive project guide
  - Project overview and conventions
  - Development workflow and testing strategy
  - WSJT-X reference implementation (build instructions and tool usage)
  - FT8 protocol reference

Failure to follow these guidelines may result in incorrect implementations or test failures.
