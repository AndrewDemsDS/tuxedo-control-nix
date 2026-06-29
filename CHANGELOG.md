# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While pre-1.0 (`0.y.z`), the public API, the NixOS module options, and ioctl behaviour
may change in any `0.y` release. See "Versioning" in the README.

## [Unreleased]

## [0.1.0] - 2026-06-27

### Added
- `tuxedoio` crate: wrappers over the `/dev/tuxedo_io` Uniwill ioctl interface (read
  temps/fan duty/mode, set fan duty, set performance profile, restore EC auto).
- `tuxedo-prober`: standalone tool to read temps/fan, drive fan duty, and toggle the
  `0x40` fan-ownership bit, used to validate the protocol on real hardware.
- `tuxedo-controld` daemon: temperature→duty fan curve with hysteresis, performance-
  profile control, a Unix control socket, and restores EC auto on `SIGTERM`.
- NixOS module `services.tuxedo-control` plus flake outputs (package, module, dev shell,
  checks, formatter).
- `tuxedo-tui`: live terminal dashboard.
- `tuxedo-gui`: libadwaita/GTK4 front-end with keyboard-backlight and charging-profile
  controls, dark-mode / follow-system colour scheme.
- Named performance profiles with their own fan curves (create/delete/default), built-in
  profiles modelled on TUXEDO Control Center, and import of TCC's exported profiles.
- Documentation: source-derived ioctl protocol map and root-cause analysis of the
  Uniwill-AMD fan bug.

### Fixed
- Loud-fan-at-idle bug on Uniwill-AMD boards: `uw_set_fan` silently ignores the
  requested duty unless bit `0x40` of EC RAM `0x0751` is clear. The daemon now clears
  `0x40` before every fan write.

### Known limitations
- Validated only on the reference machine (TUXEDO InfinityBook Pro AMD Gen9, Uniwill,
  `tuxedo_io` v0.3.9). Other boards are unverified. See "Supported hardware".
- The driver enforces a ~25% on-speed floor; a requested duty below ~12% becomes 0 (off).

[Unreleased]: https://github.com/AndrewDemsDS/tuxedo-control-nix/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/AndrewDemsDS/tuxedo-control-nix/releases/tag/v0.1.0
