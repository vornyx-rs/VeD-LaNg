# Contributing to VED

Thank you for your interest in contributing to VED! This document provides guidelines for contributing to the project.

## Code of Conduct

This project adheres to a code of conduct. By participating, you are expected to uphold this code:

- Be respectful and inclusive
- Welcome newcomers
- Focus on constructive feedback
- Accept responsibility for mistakes

## How to Contribute

### Reporting Bugs

Before creating a bug report, please:

1. Check if the issue already exists
2. Use the latest version of VED
3. Include:
   - VED version (`vedc --version`)
   - Operating system
   - Minimal code example that reproduces the issue
   - Expected vs actual behavior

### Suggesting Features

Feature requests are welcome! Please:

1. Check if the feature has already been requested
2. Describe the use case clearly
3. Consider how it fits with VED's philosophy

### Pull Requests

1. Fork the repository
2. Create a branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Add tests if applicable
5. Ensure `cargo test` passes
6. Run `cargo fmt` and `cargo clippy`
7. Commit with clear messages
8. Push to your fork and submit a PR

## Development Setup

```bash
# Clone
git clone https://github.com/vornyx-rs/VeD-LaNg
cd VeD-LaNg

# Build
cargo build

# Test
cargo test

# Run example
cargo run -- run examples/hello.ved
```

## Project Structure

```text
VeD-LaNg/
├── src/
│   ├── main.rs        -- CLI entry point
│   ├── lexer/         -- Tokenization
│   ├── parser/        -- AST construction
│   ├── ast/           -- AST definitions
│   ├── typeck/        -- Type checking
│   ├── interpreter/   -- Tree-walk interpreter
│   ├── compiler/      -- Code generation
│   ├── runtime/       -- Runtime support
│   └── stdlib/        -- Built-in definitions
├── examples/          -- Example VED programs
├── tests/             -- Integration tests
├── vscode/            -- VS Code extension
└── docs/              -- Documentation
```

## Coding Standards

### Rust Code

- Follow Rust naming conventions
- Document public APIs with rustdoc
- Handle errors explicitly (no `unwrap()` in production code)
- Write tests for new functionality

### VED Code (Examples)

- Use 2-space indentation
- Keep lines under 80 characters when possible
- Prefer descriptive variable names
- Comment complex logic

## Commit Messages

Use conventional commits format:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `test`: Tests
- `chore`: Maintenance

Example:
```
feat(lexer): add support for raw string literals

Implements #123 by adding #"..."# syntax for strings
that should not be interpolated.
```

## Testing

Run the test suite:

```bash
# All tests
cargo test

# Specific test
cargo test lexer::tests

# With output
cargo test -- --nocapture
```

## Documentation

- Update README.md if adding user-facing features
- Add rustdoc comments for public APIs
- Update CHANGELOG.md for significant changes

## Questions?

- Open an issue for questions
- Join discussions in existing issues
- Check the documentation first

Thank you for contributing to VED!
