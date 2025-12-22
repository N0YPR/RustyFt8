# Test Data for Real FT8 Recordings

This directory contains test data for validating RustyFt8 against real FT8 recordings.

## Structure

- **`recordings.json`** - Test configuration file defining expected decodes for each WAV file
- **`*.wav`** - Real FT8 audio recordings (15 seconds at 12 kHz sample rate)

## recordings.json Format

The `recordings.json` file defines test cases for real FT8 recordings. Each recording includes:

```json
{
  "recordings": [
    {
      "name": "recording_name",
      "description": "Human-readable description",
      "wav_file": "tests/test_data/filename.wav",
      "sample_rate": 12000,
      "duration_seconds": 15,
      "max_decode_time_seconds": 90,
      "messages": [
        { "text": "CQ W1ABC FN42", "required": true },
        { "text": "K1JT HA5WA 73", "required": false }
      ]
    }
  ]
}
```

### Fields

- **`name`**: Unique identifier for the recording (used in test output)
- **`description`**: Human-readable description of the recording
- **`wav_file`**: Path to the WAV file (relative to project root)
- **`sample_rate`**: Audio sample rate in Hz (should be 12000 for FT8)
- **`duration_seconds`**: Expected audio duration in seconds (should be 15)
- **`max_decode_time_seconds`**: Maximum allowed decode time (performance regression check)
- **`messages`**: Array of expected messages from WSJT-X

### Message Fields

- **`text`**: The exact message text as decoded by WSJT-X
- **`required`**: Boolean flag
  - `true` - Message MUST be decoded for test to pass (current capability)
  - `false` - Message is known to WSJT-X but not yet decoded by RustyFt8 (future goal)

## Test Behavior

The test (`test_all_real_ft8_recordings`) validates:

1. **Required messages**: All messages with `"required": true` must be decoded
2. **False positives**: No messages should be decoded that aren't in the list
3. **Optional messages**: If any `"required": false` messages are decoded, the test fails with a message to promote them to required
4. **Performance**: Decode time must not exceed `max_decode_time_seconds`

## Adding New Test Recordings

1. Obtain a real FT8 recording (15 seconds, 12 kHz sample rate)
2. Decode with WSJT-X to get reference messages
3. Add the WAV file to `tests/test_data/`
4. Add a new entry to `recordings.json`:
   - List all WSJT-X messages
   - Mark messages as `required: true` if RustyFt8 should decode them
   - Mark messages as `required: false` for future goals (very weak signals, etc.)
5. Run the test: `cargo test --release --test real_ft8_recording -- --ignored`

## Iterating on Decoder Performance

As decoder performance improves:

1. When optional messages start decoding, the test will fail with a helpful message
2. Edit `recordings.json` to change `"required": false` to `"required": true`
3. Commit the change to track progress

This ensures decoder performance never regresses while tracking progress toward WSJT-X parity.
