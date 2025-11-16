# Claude Code Generation Guidelines for Rust Projects

## CRITICAL RULES - READ FIRST

### Git Commits

**NEVER include these in commit messages:**
- ❌ "🤖 Generated with [Claude Code](https://claude.com/claude-code)"
- ❌ "Co-Authored-By: Claude <noreply@anthropic.com>"

**Commit message format:**
```
<type>: <description>

<detailed explanation>

<optional technical details>
```

## Project Overview

This template provides a foundation for developing Rust applications with Claude Code assistance. It includes best practices for project structure, code organization, testing, and documentation that align with Rust idioms and ecosystem conventions.

## Core Architecture Principles

### 1. Error Handling & Resource Management
- **Use Result types**: Prefer `Result<T, E>` over panics for recoverable errors
- **Explicit error handling**: Use `?` operator and proper error propagation
- **RAII pattern**: Rust's ownership system handles resource cleanup automatically
- **Custom error types**: Create domain-specific error types using `thiserror` or `anyhow`

```rust
// Good example
use anyhow::{Context, Result};

fn process_file(path: &str) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path))?;

    // Process content...
    Ok(content)
}
```

### 2. Concurrency & Thread Safety
- **Ownership model**: Leverage Rust's ownership system for thread safety
- **Async/await**: Use `tokio` for asynchronous programming
- **Channel communication**: Use `mpsc` channels for thread communication
- **Mutex/RwLock**: Use for shared mutable state when necessary

```rust
// Async example
use tokio::time::{sleep, Duration};

async fn fetch_data(url: &str) -> Result<String> {
    let response = reqwest::get(url).await?;
    let text = response.text().await?;
    Ok(text)
}
```

### 3. Configuration & Dependency Injection
- **Serde configuration**: Use `serde` for serialization/deserialization
- **Environment variables**: Use `dotenvy` for environment configuration
- **Dependency injection**: Pass dependencies explicitly through constructors
- **Feature flags**: Use Cargo features for conditional compilation

## File and Directory Structure

### Standard Layout
```
rust-project/
├── src/                    # Source code
│   ├── main.rs            # Binary entry point
│   ├── lib.rs             # Library entry point
│   ├── bin/               # Additional binary targets
│   ├── modules/           # Application modules
│   │   ├── mod.rs
│   │   ├── config.rs
│   │   └── utils.rs
│   └── tests/             # Integration tests
├── tests/                 # Additional integration tests
├── benches/               # Benchmarks
├── examples/              # Usage examples
├── docs/                  # Documentation
├── assets/                # Static assets
├── target/                # Build artifacts (gitignored)
├── Cargo.toml             # Project manifest
├── Cargo.lock             # Dependency lock file
└── README.md              # Project documentation
```

### File Naming Conventions
- **Rust files**: Use snake_case (e.g., `user_service.rs`, `auth_handler.rs`)
- **Test files**: Integration tests in `tests/` directory
- **Module files**: `mod.rs` for module declarations
- **Binary targets**: Place in `src/bin/` for additional executables

## Code Style & Standards

### Documentation
- **Rustdoc comments**: Use `///` for public API documentation
- **Module documentation**: Document modules with `//!` at the top
- **Examples**: Include code examples in documentation
- **Cargo.toml metadata**: Include proper project metadata

```rust
/// Processes user authentication requests.
///
/// # Arguments
///
/// * `username` - The user's username
/// * `password` - The user's password
///
/// # Returns
///
/// Returns `Ok(User)` if authentication succeeds, or `Err(AuthError)` if it fails.
///
/// # Examples
///
/// ```
/// let user = authenticate("alice", "secret123")?;
/// println!("Welcome, {}!", user.name);
/// ```
pub fn authenticate(username: &str, password: &str) -> Result<User, AuthError> {
    // Implementation...
}
```

### Logging Standards
- **Structured logging**: Use `tracing` for structured logging
- **Log levels**: Use appropriate levels (trace, debug, info, warn, error)
- **Contextual logging**: Include relevant context with spans
- **Performance**: Use logging guards for expensive operations

```rust
use tracing::{info, debug, error, instrument};

#[instrument]
async fn process_request(request_id: u64) -> Result<Response> {
    debug!("Processing request {}", request_id);

    match handle_request(request_id).await {
        Ok(response) => {
            info!("Request {} processed successfully", request_id);
            Ok(response)
        }
        Err(e) => {
            error!("Failed to process request {}: {}", request_id, e);
            Err(e)
        }
    }
}
```

### Testing Requirements
- **Unit tests**: Include `#[cfg(test)]` modules in source files
- **Integration tests**: Place in `tests/` directory
- **Property testing**: Use `proptest` for property-based testing
- **Mocking**: Use `mockall` for mocking dependencies
- **Coverage**: Use `cargo tarpaulin` for code coverage

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_basic_functionality() {
        let result = process_data("test input");
        assert!(result.is_ok());
    }

    proptest! {
        #[test]
        fn test_property_based(input in ".*") {
            let result = validate_input(&input);
            prop_assert!(result.is_ok() || result.is_err());
        }
    }
}
```

## Platform-Specific Considerations

### Cross-Platform Compatibility
- **Conditional compilation**: Use `cfg` attributes for platform-specific code
- **Path handling**: Use `std::path::Path` for cross-platform path operations
- **Feature detection**: Use `cfg!` macro for runtime feature detection

```rust
#[cfg(target_os = "windows")]
fn platform_specific_function() {
    // Windows-specific implementation
}

#[cfg(unix)]
fn platform_specific_function() {
    // Unix-specific implementation
}
```

## Common Patterns & Anti-Patterns

### Do's
- ✅ Use `Result<T, E>` for error handling
- ✅ Leverage ownership and borrowing for memory safety
- ✅ Use iterators instead of manual loops
- ✅ Implement `Display` and `Debug` traits appropriately
- ✅ Use `clippy` for code quality checks
- ✅ Write comprehensive tests and documentation
- ✅ Use `serde` for serialization needs
- ✅ Follow Rust naming conventions

### Don'ts
- ❌ Don't use `unwrap()` in production code
- ❌ Don't use `panic!` for normal error flow
- ❌ Don't ignore compiler warnings
- ❌ Don't use `unsafe` without careful consideration
- ❌ Don't create unnecessary allocations
- ❌ Don't write untested code
- ❌ Don't use global mutable state

## Development Workflow

### Feature Development
1. **Design API**: Define public interfaces and types first
2. **Write tests**: Write failing tests before implementation
3. **Implement incrementally**: Build in small, testable increments
4. **Document thoroughly**: Include examples and edge cases
5. **Commit atomically**: Make small, focused commits

### Code Review Checklist
- [ ] Follows Rust idioms and conventions
- [ ] Proper error handling with `Result` types
- [ ] Comprehensive test coverage
- [ ] Clear documentation and examples
- [ ] No compiler warnings or clippy lints
- [ ] Appropriate use of lifetimes and borrowing
- [ ] Performance considerations addressed
- [ ] Security best practices followed

## Performance Considerations

### Memory Management
- **Zero-cost abstractions**: Leverage Rust's zero-cost abstractions
- **Avoid unnecessary allocations**: Use string slices over owned strings when possible
- **Iterator chains**: Use iterator adaptors for efficient data processing
- **Profiling**: Use `perf` and `flamegraph` for performance analysis

### Async Performance
- **Async runtime**: Choose appropriate async runtime (tokio, async-std)
- **Concurrent operations**: Use `join!` and `select!` for concurrency
- **Buffering**: Use appropriate buffer sizes for I/O operations
- **Connection pooling**: Implement connection pooling for database/network operations

## Security & Privacy

### Data Handling
- **Input validation**: Validate all external inputs
- **Sanitization**: Sanitize data before processing
- **Secure defaults**: Use secure defaults for configurations
- **Secrets management**: Never hardcode secrets in source code

### Memory Safety
- **Ownership system**: Rust's ownership prevents many security issues
- **Bounds checking**: Array bounds are checked at runtime
- **Type safety**: Use strong typing to prevent logic errors
- **Unsafe code**: Minimize and carefully review any `unsafe` blocks

## Tooling & Development Environment

### Essential Tools
- **Rustfmt**: Code formatting with `cargo fmt`
- **Clippy**: Linting with `cargo clippy`
- **Cargo**: Build system and package manager
- **Rust analyzer**: IDE integration for better development experience

### Code Search & Analysis
- **Ripgrep**: Fast text search with `rg`
  - `rg "pattern"` for basic search
  - `rg -t rust "pattern"` to search only Rust files
  - `rg -A 5 -B 5 "pattern"` for context lines
- **IDE integration**: Configure your editor for Rust development

### Testing Tools
- **Cargo test**: Built-in test runner
- **Tarpaulin**: Code coverage analysis
- **Criterion**: Benchmarking framework
- **Proptest**: Property-based testing

## Common Dependencies

### Core Libraries
- **serde**: Serialization/deserialization
- **tokio**: Async runtime
- **anyhow/thiserror**: Error handling
- **tracing**: Structured logging
- **clap**: Command-line argument parsing

### Testing Libraries
- **proptest**: Property-based testing
- **mockall**: Mocking framework
- **criterion**: Benchmarking
- **tempfile**: Temporary file handling in tests

## Example Prompts for Claude

### Implementing New Features
```
Implement a REST API client for the GitHub API using reqwest and serde.
Include proper error handling, rate limiting, and comprehensive tests.
Add documentation with examples and integrate with the existing project structure.
```

### Fixing Issues
```
Fix the lifetime issue in the parser module where the returned references
don't live long enough. The compiler error is in src/parser.rs:42.
Ensure proper lifetime annotations and consider using owned types where necessary.
```

### Refactoring
```
Refactor the database module to use async/await pattern with tokio.
Convert the blocking database calls to async versions and update
all callers accordingly. Maintain backward compatibility where possible.
```

### Performance Optimization
```
Optimize the image processing pipeline for better performance.
Profile the current implementation and identify bottlenecks.
Consider using SIMD operations or parallel processing with rayon.
```

## WSJT-X Reference Implementation

### Authoritative Sources

- **FT8 Protocol Specification**: https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
- **Official Repository**: https://sourceforge.net/p/wsjt/wsjtx/ci/master/tree/
- **Source Download**: https://sourceforge.net/projects/wsjt/files/wsjtx-2.7.0/wsjtx-2.7.0.tgz/download

**Local Files**

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

**Using ft8code Tool**

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

---

This guidance ensures Claude generates idiomatic, safe, and performant Rust code that follows community best practices and modern Rust development patterns.
