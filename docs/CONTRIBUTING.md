# Contributing to eBPF Blockchain

Thank you for your interest in contributing to eBPF Blockchain! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)
- [Testing](#testing)
- [Documentation](#documentation)
- [Project Structure](#project-structure)

## Code of Conduct

### Our Pledge

We pledge to make participation in this project a welcoming and harassment-free experience for everyone, regardless of:

- Age
- Body size
- Disability
- Ethnicity
- Gender identity
- Experience level
- Nationality
- Personal appearance
- Religion
- Sexual identity

### Our Standards

**Positive behaviors include:**

- Using welcoming and inclusive language
- Being respectful of differing viewpoints and experiences
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy toward other community members

**Unacceptable behaviors include:**

- Use of sexualized language or imagery
- Trolling, insulting/derogatory comments, and personal or political attacks
- Public or private harassment
- Publishing others' private information without permission
- Other conduct which could reasonably be considered inappropriate

### Enforcement

Instances of unacceptable behavior can be reported to the project maintainers. All complaints will be reviewed and investigated promptly and fairly.

## Getting Started

### Prerequisites

Before contributing, ensure you have the following installed:

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | Nightly (latest) | Language |
| Cargo | Latest | Package manager |
| Docker | ≥ 20.10 | Monitoring stack |
| LXD | ≥ 4.0 | Container testing (optional) |
| Git | ≥ 2.30 | Version control |
| Make | Latest | Build automation (optional) |

### Setup Development Environment

```bash
# 1. Fork the repository
# 2. Clone your fork
git clone https://github.com/<your-username>/ebpf-blockchain.git
cd ebpf-blockchain

# 3. Add upstream remote
git remote add upstream https://github.com/ebpf-blockchain/ebpf-blockchain.git

# 4. Install Rust nightly
rustup default nightly
rustup component add rustfmt clippy

# 5. Build the project
cd ebpf-node
cargo build

# 6. Run tests
cargo test

# 7. Start monitoring stack (optional)
cd ../monitoring
docker-compose up -d
```

### Development Tools

```bash
# Install useful tools
cargo install cargo-watch    # Auto-reload on changes
cargo install cargo-fuzz     # Fuzzing
cargo install cargo-audit    # Security audit
```

## Development Workflow

### 1. Before You Start

1. **Check existing issues** - Make sure no one else is working on the same thing
2. **Create an issue** - If none exists, create one describing your proposed change
3. **Discuss** - Wait for maintainer feedback before starting work
4. **Get approval** - Wait for a maintainer to label the issue as `accepted`

### 2. Create a Feature Branch

```bash
# Fetch latest changes
git fetch upstream

# Create branch from main
git checkout -b feature/<feature-name> upstream/main

# Or for bug fixes
git checkout -b fix/<bug-description> upstream/main
```

**Branch Naming Conventions:**

| Type | Format | Example |
|------|--------|---------|
| Feature | `feature/<name>` | `feature/replay-protection` |
| Bug fix | `fix/<description>` | `fix/nonce-validation` |
| Documentation | `docs/<topic>` | `docs/api-documentation` |
| Testing | `test/<scope>` | `test/integration-network` |
| Chore | `chore/<task>` | `chore/cargo-update` |

### 3. Make Your Changes

- Write code following the coding standards
- Write tests for your changes
- Update documentation as needed
- Test locally before committing

### 4. Commit Your Changes

```bash
git add <files>
git commit -m "feat: add replay protection for transactions"
```

See [Commit Guidelines](#commit-guidelines) for format.

### 5. Push and Create Pull Request

```bash
git push origin feature/<feature-name>
```

Then go to GitHub and create a Pull Request.

## Coding Standards

### Rust Style

We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and use:

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy --all-targets --all-features

# Run security audit
cargo audit
```

### Code Organization

```rust
// 1. Imports (std first, then external, then project)
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::types::Transaction;

// 2. Constants
const MAX_RETRIES: u32 = 3;

// 3. Structs
pub struct MyComponent {
    field: String,
}

// 4. Implementations
impl MyComponent {
    // Public API first
    pub fn new() -> Self { ... }
    
    // Private methods last
    fn helper_method(&self) { ... }
}

// 5. Tests
#[cfg(test)]
mod tests { ... }
```

### Error Handling

Use `anyhow::Result` for application-level errors and thiserror for library errors:

```rust
use anyhow::{Result, Context};

pub fn my_function() -> Result<()> {
    do_something()
        .context("Failed to do something")?;
    Ok(())
}
```

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Variables | `snake_case` | `peer_id`, `block_height` |
| Functions | `snake_case` | `validate_transaction()` |
| Structs | `PascalCase` | `Transaction`, `ReplayProtection` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES` |
| Modules | `snake_case` | `consensus`, `network` |
| Files | `snake_case` | `main.rs`, `replay_protection.rs` |

### Documentation Comments

```rust
/// Validates a transaction nonce to prevent replay attacks.
///
/// # Arguments
/// * `sender` - The sender's peer ID
/// * `nonce` - The transaction nonce
///
/// # Returns
/// * `Ok(u64)` - The validated nonce
/// * `Err(String)` - If nonce is invalid
pub fn validate_nonce(&self, sender: &str, nonce: u64) -> Result<u64, String> {
    // Implementation
}
```

## Commit Guidelines

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `style` | Code style (formatting, semicolons) |
| `refactor` | Code refactoring |
| `test` | Adding/modifying tests |
| `chore` | Maintenance tasks |
| `perf` | Performance improvements |
| `ci` | CI/CD changes |
| `build` | Build system changes |

### Examples

```bash
feat(consensus): add proof of stake validation
fix(network): resolve peer connection timeout
docs(api): update endpoint documentation
style(ebpf): format code with cargo fmt
refactor(security): extract replay protection module
test(integration): add network propagation tests
chore(deps): update cargo dependencies
perf(storage): optimize RocksDB compaction
```

## Pull Request Process

### Before Submitting

1. **Update documentation** - Include changes in relevant docs
2. **Add tests** - Cover new functionality
3. **Run all tests** - Ensure nothing is broken
4. **Update CHANGELOG** - Add entry under `Unreleased`

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation update
- [ ] Refactoring
- [ ] Testing

## Testing
- [ ] `cargo test` passes
- [ ] `cargo fmt` applied
- [ ] `cargo clippy` passes
- [ ] Manual testing performed

## Screenshots (if applicable)
Screenshots of UI changes

## Checklist
- [ ] My code follows the style guidelines
- [ ] I have performed a self-review
- [ ] I have commented my code
- [ ] I have updated documentation
- [ ] I have added tests
```

### Review Process

1. **Automated checks** - CI must pass
2. **Maintainer review** - At least one maintainer approval
3. **Squash merge** - PRs are squashed and merged to main

### Review Timeline

| Stage | Expected Time |
|-------|---------------|
| Initial review | 48 hours |
| Review comments | 24 hours per round |
| Final approval | 72 hours |

## Testing

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*'

# Specific test
cargo test --test network_test test_peer_connection

# With output
cargo test -- --nocapture

# All tests including ignored
cargo test -- --include-ignored
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_feature_works() {
        // Arrange
        let component = MyComponent::new();
        
        // Act
        let result = component.do_something().await;
        
        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_value);
    }
    
    #[test]
    #[ignore] // For expensive tests
    fn test_integration_scenario() {
        // Integration test
    }
}
```

### Test Coverage

Target: **> 80%** code coverage

```bash
# Install cargo-tarpaulin
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html

# Open report
open tarpaulin-report.html
```

## Documentation

### Types of Documentation

| Type | Location | Purpose |
|------|----------|---------|
| README | `README.md` | Project overview |
| API | `docs/API.md` | API documentation |
| Architecture | `docs/ARCHITECTURE.md` | System design |
| Operations | `docs/OPERATIONS.md` | Runbook |
| ADRs | `docs/adr/` | Design decisions |
| Code | `///` comments | Inline documentation |

### Generating Documentation

```bash
# Rustdoc
cargo doc --open

# With private items
cargo rustdoc -- --document-private-items
```

### Documentation Checklist

- [ ] README updated with changes
- [ ] API docs updated if endpoints changed
- [ ] Architecture diagram updated if structure changed
- [ ] ADR created for significant decisions
- [ ] CHANGELOG updated
- [ ] Comments added for public APIs

## Project Structure

```
ebpf-blockchain/
├── ebpf-node/                    # Main Rust project
│   ├── ebpf-node/               # User space binary
│   │   ├── src/
│   │   │   ├── main.rs          # Entry point
│   │   │   ├── consensus/       # Consensus module
│   │   │   ├── network/         # P2P networking
│   │   │   ├── security/        # Security modules
│   │   │   ├── storage/         # Storage module
│   │   │   └── metrics/         # Metrics module
│   │   ├── tests/               # Integration tests
│   │   └── Cargo.toml
│   ├── ebpf-node-ebpf/          # eBPF programs
│   │   ├── src/
│   │   │   ├── main.rs          # XDP program
│   │   │   └── lib.rs           # KProbes/Tracepoints
│   │   └── Cargo.toml
│   └── ebpf-node-common/        # Shared types
├── monitoring/                   # Observability stack
├── ansible/                      # Deployment automation
├── docs/                         # Documentation
├── scripts/                      # Utility scripts
├── tests/                        # Additional tests
└── .github/                      # CI/CD workflows
```

## Reporting Bugs

### Before Reporting

1. **Search existing issues** - Check if the bug is already reported
2. **Check troubleshooting** - See [docs/OPERATIONS.md](docs/OPERATIONS.md)
3. **Reproduce** - Confirm the bug is reproducible

### Bug Report Template

```markdown
**Describe the bug**
Clear description of the bug

**To Reproduce**
Steps to reproduce:
1. Start node with config X
2. Submit transaction Y
3. Observe error Z

**Expected behavior**
What should happen

**Actual behavior**
What actually happens

**Logs**
```
journalctl -u ebpf-blockchain -n 100
```

**Environment**
- OS: Ubuntu 22.04
- Kernel: 5.15
- Rust: nightly-2026-01-15
```

## Suggesting Features

### Feature Request Template

```markdown
**Problem**
Clear description of the problem

**Solution**
Proposed solution

**Alternatives Considered**
Other solutions considered

**Additional Context**
Screenshots, examples, references
```

## Release Process

### Version Numbering

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR** - Incompatible API changes
- **MINOR** - Backwards-compatible functionality
- **PATCH** - Backwards-compatible bug fixes

### Release Checklist

- [ ] All tests passing
- [ ] CHANGELOG updated
- [ ] Version bumped
- [ ] Documentation updated
- [ ] Release notes written
- [ ] CI/CD passing
- [ ] Maintainer approval

## Questions?

If you have questions:

- Check [docs/](docs/) for documentation
- Search [GitHub Issues](https://github.com/ebpf-blockchain/ebpf-blockchain/issues)
- Create a new issue with the `question` label

---

Thank you for contributing to eBPF Blockchain! 🚀
