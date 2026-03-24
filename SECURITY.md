# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| latest  | Yes       |

## Reporting a Vulnerability

If you discover a security vulnerability in git-paw, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please use [GitHub Security Advisories](https://github.com/bearicorn/git-paw/security/advisories/new) to report vulnerabilities privately.

### What to include

- Description of the vulnerability
- Steps to reproduce
- Impact assessment
- Suggested fix (if any)

### Response timeline

- **Acknowledgment**: within 48 hours
- **Initial assessment**: within 1 week
- **Fix or mitigation**: as soon as feasible, depending on severity

## Scope

git-paw is a CLI tool that orchestrates tmux sessions and git worktrees. Security concerns most likely involve:

- Command injection via branch names or CLI arguments
- Unsafe file operations (symlink attacks, path traversal)
- Insecure handling of configuration files

Thank you for helping keep git-paw safe.
