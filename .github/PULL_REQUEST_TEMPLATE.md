## Summary

<!-- What does this PR do and why? Link the issue it closes. -->

Closes #

## Checklist

- [ ] Commits use Conventional Commits
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] `nix flake check` passes (if `nix/` or the module changed)
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] New `unsafe` is only in `crates/tuxedoio` with a `// SAFETY:` comment

## Hardware validation (required for fan / EC / ioctl changes)

- Machine / board:
- `tuxedo_io` version + kernel:
- What you tested, what you observed (and that the EC was returned to `auto`):
