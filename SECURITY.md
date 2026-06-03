# Security Policy

## Supported versions

SlotForge is pre-1.0. Security fixes are applied on the default branch (`main`).
Release tags, when published, will receive backports at maintainers' discretion.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, use one of these channels:

1. **GitHub Security Advisories** (preferred):  
   [Report a vulnerability](https://github.com/squareguard/SlotForge/security/advisories/new)  
   on the repository (Private vulnerability reporting, if enabled).

2. **Email:** **joe@squareguard.co.uk** with a subject line like `SlotForge security`.

Include:

- A description of the issue and impact
- Steps to reproduce or a proof of concept
- Affected version or commit hash
- Your environment (OS, build type: CLI vs Tauri desktop)

We aim to acknowledge reports within **5 business days** and will work with you on
a fix and coordinated disclosure when appropriate.

## Scope

In scope:

- Path traversal or unauthorized filesystem access via Tauri IPC or Rust APIs
- Vault delete / swap / backup operations escaping configured roots
- Integrity bypass (hash verification, rollback) in swap flows
- Sensitive data exposure in logs, audit files, or UI

Out of scope (unless combined with exploitable impact):

- Issues that require the attacker to already control the local user account
- Denial of service from very large local save files without a practical attack path
- Third-party game installers or modded game directories

## Safe harbor

We appreciate responsible disclosure and will not pursue legal action against
researchers who follow this policy in good faith.
