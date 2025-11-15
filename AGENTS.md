# AGENTS.md - Guide for AI Assistants

This file helps AI assistants understand the RustyFt8 project structure, conventions, and workflows.

## Project Overview

RustyFt8 is a `no_std` Rust library for encoding and decoding FT8 amateur radio messages. It implements the complete 77-bit FT8 message protocol compatible with WSJT-X.

**Key characteristics:**
- `no_std` compatible (embedded systems)
- Zero dependencies on standard library
- Uses `alloc` for String/Vec types
- Bit manipulation with `bitvec` crate
- Comprehensive test coverage (116 tests, 58 CSV-driven roundtrip cases)

## Code Conventions

### Documentation
- All public functions must have doc comments
- Module-level docs (`//!`) explain purpose and format
- Examples in doc comments should use `no_run` for no_std compatibility

## Development Workflow

### Testing Against WSJT-X

```bash
# Encode a message with WSJT-X reference
./scripts/ft8code.sh "CQ N0YPR DM42"

# Add as test case
./scripts/add_test_case.sh "CQ N0YPR DM42"

# Run roundtrip tests
cargo test message::tests::test_encode_decode_roundtrip
```

### Git Workflow

- **Branch**: `development` (main development branch)
- **Commits**: Must be approved before execution
- **Commit messages**:
  - Use conventional format
  - Do not include "🤖 Generated with Claude Code" footer
  - Do not include "Co-Authored-By: Claude <noreply@anthropic.com>"
  
### Common Commands

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_encode_decode_roundtrip

# Build documentation
cargo doc --no-deps --open

# Run with main.rs
cargo run -- encode "CQ N0YPR DM42"

# Check compilation for embedded targets
cargo check --target thumbv7em-none-eabihf
```

## FT8 Protocol Reference

### Message Types (i3 field)
- **Type 0**: Free text, telemetry, DXpedition mode
- **Type 1**: Standard QSO (with /R suffix support)
- **Type 2**: EU VHF Contest (with /P suffix)
- **Type 3**: RTTY Roundup, ARRL Field Day
- **Type 4**: Non-standard callsigns (compound callsigns)

### Bit Layout
- **77 bits total** (source message)
- **Bits 74-76**: i3 (message type)
- **Bits 71-73**: n3 (subtype)
- **Remaining bits**: Message-specific fields

### Key Encodings
- **Callsign**: 28 bits (base-37 packing)
- **Grid square**: 15 bits (AA00 format)
- **Signal report**: -30 to +99 dB
- **Hash values**: 10-bit, 12-bit, 22-bit

## Known Issues & Limitations

- WSPR messages (Type 0.6) not implemented - out of scope for FT8
- Some edge cases in non-standard callsign validation
- Documentation warnings about HTML tags in Field Day format strings

## WSJT-X Reference Implementation

### Authoritative Sources

- **FT8 Protocol Specification**: https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
- **Official Repository**: https://sourceforge.net/p/wsjt/wsjtx/ci/master/tree/
- **Source Download**: https://sourceforge.net/projects/wsjt/files/wsjtx-2.7.0/wsjtx-2.7.0.tgz/download

### Local Files

The WSJT-X source is available in the `./wsjtx/` directory (added to `.gitignore`):
- **Source code**: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/77bit/`
- **Test messages**: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/77bit/messages.txt`
- **Encoder/decoder**: `packjt77.f90`, `unpackjt77.f90`

### Building WSJT-X Tools

The devcontainer includes all necessary dependencies. To build WSJT-X tools:

```bash
# Download and extract (if not present)
cd /workspaces/RustyFt8/wsjtx
wget https://sourceforge.net/projects/wsjt/files/wsjtx-2.7.0/wsjtx-2.7.0.tgz/download -O wsjtx-2.7.0.tgz
tar -xzf wsjtx-2.7.0.tgz
cd wsjtx-2.7.0

# Configure (skip docs to avoid build issues)
mkdir -p build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release -DWSJT_GENERATE_DOCS=OFF -DWSJT_SKIP_MANPAGES=ON

# Build (uses all CPU cores)
make -j$(nproc)

# Verify build artifacts
ls -lh wsjtx-prefix/src/wsjtx-build/ | grep -E "ft8|jt9"
```

**Key build artifacts** in `build/wsjtx-prefix/src/wsjtx-build/`:
- **`ft8code`** - FT8 encoder utility (used by test scripts)
- **`ft8sim`** - FT8 signal simulator for testing
- **`jt9`** - Decoder for FT8 and other modes
- **`wsjtx`** - Main WSJT-X application

### Using ft8code Tool

The `ft8code` utility is essential for validating implementations:

```bash
# Encode a message and show details
./wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code "CQ N0YPR DM42"

# Show all message type examples
./wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code -T
```

**Output includes**:
- **Source-encoded message**: 77-bit payload
- **14-bit CRC**: Error detection
- **83 Parity bits**: LDPC forward error correction
- **Channel symbols**: 79 tones (8-FSK, 0-7) for complete transmission

The project includes wrapper scripts:
- `./scripts/ft8code.sh "MESSAGE"` - Simplified wrapper
- `./scripts/add_test_case.sh "MESSAGE"` - Add to test suite

## Tips for AI Assistants

1. **Always run tests** after changes: `cargo test`
2. **Check no_std compatibility**: Avoid `std::` imports, use `alloc::`
3. **Preserve test coverage**: Add tests for new functionality
4. **Follow naming conventions**: Match existing module structure
5. **Ask before major refactoring**: Confirm approach with user
6. **Use WSJT-X as reference**: When in doubt, check `packjt77.f90`
7. **Git commits**: Always get approval before committing
8. **Test against WSJT-X**: Use `./scripts/ft8code.sh` to verify compatibility

## Questions to Ask

When unclear about implementation:
- "Should this follow WSJT-X exactly, or can we simplify?"
- "Is this message type in scope for FT8, or is it WSPR/other?"
- "Do we need backward compatibility with previous versions?"
- "Should this be public API or internal?"
