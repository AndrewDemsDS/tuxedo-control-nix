# Contributing

Thanks for considering a contribution. This is a small but serious project: a
declarative TUXEDO laptop hardware-control tool built by reverse-engineering the
`tuxedo_io` kernel ioctl interface. Because it drives a **root daemon that writes to
embedded-controller (EC) registers**, correctness and honesty about hardware support
matter more than feature velocity. Please read the hardware and safety sections before
opening a PR.

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## TL;DR

```sh
nix develop                       # dev shell: rustc, cargo, clippy, rustfmt, gtk4, strace
nix build .#default               # build the package
nix flake check                   # evaluate the module + run checks

# or plain cargo
cargo build --release             # workspace: tuxedoio, prober, daemon, tui, gui
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Commit messages follow [Conventional Commits](#commit-conventions).

## Project layout

| Crate | Binary | Role |
|---|---|---|
| `crates/tuxedoio` | _(lib)_ | Wrappers over the `/dev/tuxedo_io` Uniwill ioctls. The only place that touches hardware. |
| `prober/` | `tuxedo-prober` | Read temps/fan, set duty, toggle the `0x40` fan-ownership bit. |
| `daemon/` | `tuxedo-controld` | Temp→duty fan curve, hysteresis, profiles, restores EC auto on `SIGTERM`. |
| `tui/` | `tuxedo-tui` | Live terminal dashboard. |
| `gui/` | `tuxedo-gui` | libadwaita/GTK4 front-end; talks to the daemon over a socket. |
| `nix/` | (none) | `module.nix` (`services.tuxedo-control`) + `package.nix`. |
| `docs/` | (none) | Source-derived protocol notes: the authoritative record of each ioctl. |

**Golden rule:** all hardware access goes through `crates/tuxedoio`. Don't open
`/dev/tuxedo_io` or issue raw ioctls from `daemon`/`tui`/`gui`. Add a typed method to
the library and call it.

## Running and testing on hardware

Most of this only does anything as **root on a TUXEDO/Uniwill laptop**. Be careful:
you are writing to the EC.

```sh
sudo ./target/release/tuxedo-prober info     # read-only; safe
sudo ./target/release/tuxedo-prober set 40   # drive both fans to 40%
sudo ./target/release/tuxedo-prober auto     # ALWAYS hand control back to the EC when done
```

Safety checklist before testing fan code:

1. Keep `tuxedo-prober auto` ready (restores EC automatic control).
2. Watch temperatures (`tuxedo-tui` in another pane, or `sensors`).
3. Never merge a path that can leave the fan **off** without a temperature safety floor.
   The daemon must restore EC auto on exit (`SIGTERM`/`SIGINT`).
4. Disable other fan controllers (`tailord`, `tccd`) first. Two daemons fighting over
   `/dev/tuxedo_io` produce nonsense.

Pure logic (curve interpolation, hysteresis, duty scaling, config parsing) must be
unit-tested without hardware; keep it free of ioctl calls so `cargo test` covers it.
Hardware-dependent behaviour can't run in CI; describe your manual validation in the PR.

## Code style

- `cargo fmt --all` (CI enforces `--check`).
- `cargo clippy --workspace --all-targets -- -D warnings`: no new warnings.
- `unsafe` only in `crates/tuxedoio`, only at the ioctl FFI boundary, with a `// SAFETY:`
  comment.
- EC offsets / ioctl numbers / scaling constants (`0xc8`, `0x40`, `0x0751`, …) must be
  named constants with a comment citing the source (driver file or `docs/`). We are
  source-first; never bake in a guessed constant.

## Commit conventions

[Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`.
Types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`.
Scopes: `daemon`, `prober`, `tui`, `gui`, `tuxedoio`, `nix`, `module`, `docs`. A `!`
(or `BREAKING CHANGE:` footer) marks a breaking change. Add a line to the `[Unreleased]`
section of [CHANGELOG.md](CHANGELOG.md) for user-facing changes.

## Adding support for a new board

The most valuable contribution, and the highest-responsibility one. Follow the
source-first method (don't reverse the wire blind):

1. **Open an issue first** with `sudo dmidecode -s system-product-name`, board name,
   SoC, `tuxedo_io` version (`tuxedo-prober info`), kernel, and `lsmod | grep -E 'uniwill|clevo'`.
2. **Read the driver source** (`tuxedo-drivers`, `src/tuxedo_io/`) for the ioctl numbers
   and EC offsets. Cite what you read.
3. **Trace** the reference daemons to confirm the real sequence:
   `sudo strace -f -e trace=ioctl -p "$(pgrep -x tailord)"`.
4. **Validate with the prober** before touching the daemon: reads first, then writes
   with `auto` as the safety net.
5. **Gate by model** (`R_UW_MODEL_ID`) where behaviour differs; don't change the default
   path for boards you can't test.
6. **Document** in `docs/` and update the [supported-hardware table](README.md#supported-hardware).
   A board is only marked supported once fan control is validated on it.

If you can't test on hardware, protocol notes, doc fixes, the GUI/TUI, the Nix module,
and tests are all welcome and lower-risk.

## Pull requests

1. Topic branch, one logical change per PR.
2. Run the gate locally: `cargo fmt --all -- --check`, `cargo clippy --workspace
   --all-targets -- -D warnings`, `cargo test --workspace`, and `nix flake check` if you
   touched `nix/`.
3. Update `CHANGELOG.md` and any affected docs.
4. For hardware changes, include your manual validation (machine, board, `tuxedo_io`
   version, observed behaviour).

Security-relevant issues: do **not** open a public issue. See [SECURITY.md](SECURITY.md).

## License

By contributing you agree your contributions are licensed under the project's
[MIT License](LICENSE).
