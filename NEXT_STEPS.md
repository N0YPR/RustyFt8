# Next Steps for RustyFT8

## Current Status

RustyFT8 now has a complete FT8 encoder implementation matching WSJT-X:

✅ **Encoder Features**:
- Full message encoding pipeline (text → 77-bit message → CRC-14 → LDPC → symbols → audio)
- Support for all FT8 message types (Standard, EU VHF, ARRL contests, etc.)
- Callsign hash cache populated during both encode and decode (matching WSJT-X)
- High-level encoder API (`encode_ft8`, `encode_ft8_audio`)
- Binary utilities: `ft8code`, `ft8sim`, `ft8detect`

✅ **Test Coverage**:
- 191 tests passing
- Comprehensive coverage of message types
- Known WSJT-X test vectors

## Potential Future Work

### 1. UI Integration
The library is ready for integration into a user-facing application:
- Core library in `src/lib.rs` exposes clean API
- Encoder functions available via `rustyft8::encoder::{encode_ft8, encode_ft8_audio}`
- Decoder available via `rustyft8::decode_ft8_audio`

### 2. Decoder Performance Tuning (Optional)
The decoder works but could potentially be optimized to better match WSJT-X performance:
- LDPC parameter tuning
- Multi-symbol combining optimization
- OSD enhancements (see archived investigation docs)

### 3. Documentation
- API documentation (Rustdoc)
- Usage examples for common scenarios
- Integration guide for UI developers

### 4. Additional Features
- Real-time audio input/output
- QSO logging integration
- Contest mode support
- Additional message type support (if new types are added to FT8 spec)

## Reference Implementation

WSJT-X source code is available in `./wsjtx/` directory for reference:
- **Encoder**: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/77bit/packjt77.f90`
- **Decoder**: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/77bit/unpackjt77.f90`
- **Test messages**: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/77bit/messages.txt`

## Testing

Run all tests:
```bash
cargo test --release
```

Test encoder with specific message:
```bash
cargo run --bin ft8code "CQ N0YPR DM42"
```

Test decoder on real recording:
```bash
cargo test --release --test real_ft8_recording -- --ignored --nocapture
```

Compare with WSJT-X:
```bash
./wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code "CQ N0YPR DM42"
./wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 tests/test_data/210703_133430.wav
```

## Project Structure

```
RustyFT8/
├── src/
│   ├── lib.rs              # Library API
│   ├── encoder.rs          # High-level encoder
│   ├── message/            # Message encoding/decoding
│   ├── ldpc/               # LDPC forward error correction
│   ├── symbol/             # Symbol mapping
│   ├── sync/               # Signal synchronization
│   └── bin/
│       ├── ft8code.rs      # Message encoder utility
│       ├── ft8sim.rs       # Signal simulator
│       └── ft8detect.rs    # Signal decoder
├── tests/                  # Integration tests
├── examples/               # Usage examples
└── wsjtx/                  # WSJT-X reference (gitignored)
```