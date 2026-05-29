# SlotForge Cross-Platform Fixtures

This directory documents fixture conventions used for filesystem and path behavior tests.

## Goals

- Keep tests deterministic across Windows, Linux, and macOS.
- Verify path handling behavior without depending on machine-specific folders.
- Ensure save copy/swap workflows are validated with realistic file trees.

## Fixture Rules

- Use temporary directories for writable fixtures; do not commit mutable binary fixtures.
- Prefer tiny text payloads to represent save files in unit tests.
- Keep names ASCII-only and include at least one case with spaces.
- Include both "active save" and "vault save" fixture layouts when testing swap behavior.

## Path Cases To Cover

- Windows-style absolute path and `%VAR%` expansion.
- Unix-style absolute path and `${VAR}` expansion.
- Relative path normalization (`.` / `..`) behavior.
- Mixed-separator input normalization where applicable.
