# Reverse-engineering plan

Source-first steps. The aim: a correct understanding of `/dev/tuxedo_io` on
Uniwill-AMD boards, then a minimal daemon + NixOS module.

## Upstream sources (the source-of-truth set)

- **tuxedo-drivers** (GPL kernel modules): defines the ioctl request numbers + structs.
  `https://gitlab.com/tuxedocomputers/development/packages/tuxedo-drivers`
  Look at `src/tuxedo_io/tuxedo_io_ioctl.h` and the Uniwill (`uw_`) fan paths.
- **tuxedo-rs** (Rust, MIT): `tailord` + `tailor_gui`.
  `https://github.com/AaronErhardt/tuxedo-rs`. The `tuxedo_ioctl` crate wraps
  the same ioctls; this is the base to fix.
- **tuxedo-control-center** (Electron + `tccd`, GPL).
  `https://github.com/tuxedocomputers/tuxedo-control-center` holds `tccd`'s fan logic for the
  Uniwill path; diff against tuxedo-rs to find the discrepancy.
- **blitz/tuxedo-nixos**: existing community Nix packaging; reference for module shape.

## Phase 1: map the ioctl protocol

1. **Read the numbers from the driver**, don't guess:
   ```sh
   # in a checkout of tuxedo-drivers
   rg -n 'TUXEDO_IO|_IOR|_IOW|_IOWR|uw_set_fan|uw_get_fan|fan_speed' src/tuxedo_io
   ```
   Record each request macro, direction, and the struct it carries.
2. **Trace the real daemons** on this board (needs root; `tailord` re-enabled temporarily):
   ```sh
   sudo strace -f -e trace=ioctl -s 256 -p "$(pgrep -x tailord)" 2>&1 | rg -i 'tuxedo|ioctl'
   ```
   Capture the exact request numbers + arg bytes for: read temp, read fan RPM, set fan
   duty, set performance profile. Repeat while toggling profiles in `tailor_gui`.
3. *(If obtainable)* do the same against `tccd` to get the vendor reference sequence.
4. **Answer the key question**: is there an ioctl/EC write tied to the performance profile
   that forces a fan-duty floor? (Reference machine: `enthusiast` → loud fan regardless of
   the curve; `power_save` over D-Bus didn't help.) This is the bug's root cause.

## Phase 2: `prober/`

A tiny standalone tool (Rust preferred; C acceptable) that:
- opens `/dev/tuxedo_io`,
- reads CPU/GPU temp + fan RPM via the mapped ioctls,
- sets fan duty to an explicit percent (and to "auto"),
- prints everything.

Success criterion: **drive the fan to 0 % at a cold idle and watch it ramp under load**,
proving direct control works (independent of any performance-profile coupling). This is the
experiment that validates Phase 1's findings before any daemon work.

## Phase 3: daemon

Minimal long-running controller:
- temp→duty curve from config (decoupled from the performance profile, the fix),
- hysteresis + a hard ramp near crit,
- D-Bus or a tiny socket for runtime overrides,
- set the performance profile *without* letting it dictate fan duty.

Prefer reusing the `tuxedo_ioctl` crate from tuxedo-rs if its Uniwill path is correct; else
patch it and upstream the fix.

## Phase 4: NixOS module + flake (`nix/`)

- `flake.nix` outputs: `packages.tuxedo-control` (daemon), `nixosModules.default`.
- `services.tuxedo-control` options: `enable`, `fan.curve`, `performanceProfile`,
  `chargeLimit`, `keyboard.backlight`.
- Module wires the daemon as a systemd service, depends on the `tuxedo_io` module being
  loaded (assert / `boot.kernelModules`), and stays **declarative** (no stateful
  `/etc/tailord` rewriting).
- CI: build the daemon + `nix flake check`, plus a VM test that loads the module.

## Phase 5 (optional)

A TUI (ratatui) or small GTK GUI; matugen-themeable to match the desktop.

## Non-goals

- Re-packaging the Electron TCC verbatim (avoiding that is the point).
- Supporting every TUXEDO model on day one. Start with Uniwill-AMD (Gen9), generalise later.
