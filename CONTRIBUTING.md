# Contributing to SlotForge

Thank you for your interest in contributing. SlotForge is a cross-platform desktop
app for managing PC game saves; contributions that improve reliability, UX, and
cross-platform behavior are especially welcome.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By
participating, you agree to uphold it. Report concerns to **joe@squareguard.co.uk**.

## Ways to contribute

- **Bug reports** — Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.yml) and include your OS, Rust/Node versions, and steps to reproduce.
- **Feature ideas** — Open a [feature request](.github/ISSUE_TEMPLATE/feature_request.yml) so we can discuss scope before large PRs.
- **Pull requests** — Fixes, tests, docs, and small features are great. For larger changes, open an issue first.
- **Security issues** — Do **not** open a public issue. See [SECURITY.md](SECURITY.md).

## Development setup

1. **Fork and clone** the repository.
2. **Install tooling:**
   - [Rust](https://rustup.rs/) (stable; 1.70+ recommended)
   - [Node.js](https://nodejs.org/) 18+ and npm
3. **Install dependencies:**

   ```bash
   npm install
   cd frontend && npm install && cd ..
   ```

4. **Run the desktop app** (recommended for full IPC behavior):

   ```bash
   npm run tauri:dev
   ```

5. **Run the Rust CLI** (headless / diagnostics):

   ```bash
   cargo run
   ```

Optional isolated API self-test:

```bash
# PowerShell
$env:SLOTFORGE_SELF_TEST = "1"
cargo run
```

Override config path for local testing:

```bash
# PowerShell
$env:SLOTFORGE_CONFIG_PATH = "C:\path\to\config.json"
cargo run
```

See [README.md](README.md) for architecture, project layout, and configuration paths.

## Before you open a PR

Run the same checks as CI on your machine:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

On Windows, PowerShell:

```powershell
$env:SLOTFORGE_CONFIG_PATH = "$PWD\.tmp\slotforge-test-config.json"
cargo test --all-targets
```

CI runs these steps on **Ubuntu, Windows, and macOS** (see
[`.github/workflows/cross-platform-tests.yml`](.github/workflows/cross-platform-tests.yml)).

## Pull request guidelines

- **One logical change per PR** when possible (easier to review and bisect).
- **Describe what and why** in the PR summary; note breaking changes and manual test steps.
- **Match existing style** — Rust formatted with `rustfmt`, no new `clippy` warnings, React/JS consistent with `frontend/src/`.
- **Add or update tests** when fixing bugs or changing service behavior under `src/services/` or `src/api/`.
- **Avoid unrelated drive-by refactors** in the same PR as a feature fix.

## Project areas (where to start)

| Area | Path | Notes |
|------|------|--------|
| Core logic | `src/services/` | Backup, vault, swap, discovery, config |
| Tauri IPC | `src-tauri/src/`, `src/api/` | Commands exposed to the UI |
| React UI | `frontend/src/` | `SlotForgeApp.jsx`, hooks, `slotforgeApi.js` |
| Platform | `src/platform/` | OS paths and filesystem helpers |
| Tests | `tests/`, `#[cfg(test)]` in crates | Use fixtures under `tests/fixtures/` |

## Commit messages

Use clear, imperative subjects (e.g. `Fix vault delete path validation on Windows`).
Reference issue numbers when applicable (`Fixes #12`).

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE), the same license as the project.

## Questions

Open a [GitHub Discussion](https://github.com/squareguard/SlotForge/discussions) or
an issue labeled **question** if you are unsure where a change should live. We are
happy to point you at the right module before you invest in a large patch.
