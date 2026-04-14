# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 1.0.x   | Yes       |
| < 1.0   | No        |

## Reporting a Vulnerability

If you discover a security vulnerability in VED, please report it responsibly.

**Do NOT open a public issue.**

Email `security@ved-lang.org` with:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Response Timeline

- Acknowledgment within 48 hours
- Initial assessment within 1 week
- Fix timeline communicated based on severity

### Known Accepted Advisories (v1.0.0)

The following advisories are acknowledged and accepted for v1.0.0. Details and
justification are in `audit.toml`.

| ID | Crate | Reason accepted |
| --- | --- | --- |
| RUSTSEC-2023-0071 | rsa 0.9.x | Via sqlx-macros-core. No upstream fix. vedc does not perform RSA operations. |
| RUSTSEC-2024-0436 | paste 1.x | Unmaintained; pulled in by optional wgpu feature only. No security impact. |
| RUSTSEC-2026-0097 | rand 0.8.5 | Unsound with custom global logger. vedc installs no custom logger. |

## Security Best Practices for VED Applications

1. Keep the compiler updated — `cargo install --path . --force`
2. Do not expose stack traces or internal errors to end users in server builds
3. Validate all external input in `think` handlers before passing to `db` operations
4. Use `env "SECRET"` for credentials — never hardcode secrets in `.ved` source
5. Use HTTPS for server applications in production
6. Run `cargo audit` on generated server `Cargo.toml` before deploying

## Acknowledgments

We thank the security researchers who help keep VED and its users safe.
