# Hardware interface: recon notes (Phase 0)

Captured on the reference machine. Every value below is read from the running system,
not guessed. These are the starting points for the reverse-engineering work.

## Reference machine

| | |
|---|---|
| Vendor / product | `TUXEDO` / **InfinityBook Pro AMD Gen9** |
| Board name | `GXxHRXx` (Uniwill barebone) |
| SoC | AMD Ryzen 7 8845HS (Radeon 780M) |
| Kernel (tested) | Linux 7.1 (NixOS `linuxPackages_latest`); also seen on 6.18 |

## Kernel modules in play

```
tuxedo_io        0.3.9   <- the ioctl interface (the thing to RE)
tuxedo_keyboard          <- keyboard backlight + platform glue
uniwill_wmi              <- THIS board's WMI path (AMD Gen9 is Uniwill, not Clevo)
clevo_wmi / clevo_acpi   <- loaded but Clevo path (not the active one here)
tuxedo_compatibility_check
```

The board is **Uniwill** (`uniwill_wmi`). The fan/EC control path runs through the Uniwill
interface, which matters because tuxedo-rs/tailord's Uniwill AMD handling holds the bug.

## The control interface

- Char device: `/dev/tuxedo_io` (`crw------- root root 237, 0`)
- Provided by: `tuxedo_io.ko` v0.3.9, `srcversion F320AC0977D2444CD90BC11`
  (from the `tuxedo-drivers` package / `updates/src/tuxedo_io/`).
- Both `tccd` (TCC) and `tailord` (tuxedo-rs) open this device and issue `ioctl()` calls.
  Phase 1 maps this one chokepoint.

## tailord (tuxedo-rs) observed behaviour: the bug to fix

- Connects fine: log `Connected to Tuxedo ioctl interface with version 0.3.9`.
- Reads the fan curve from `/etc/tailord/fan/<profile>.json`, e.g. `default.json`:
  a list of `{ "temp": <°C>, "fan": <percent> }` points. The reference quiet curve kept
  fan `0 %` below `49–50 °C`.
- D-Bus surface (system bus, `com.tux.Tailor`, object `/com/tux/Tailor`):
  - `com.tux.Tailor.Performance`: `ListProfiles` → `["power_save","enthusiast","overboost"]`,
    `GetProfile`, `SetProfile(s)`.
  - `com.tux.Tailor.Fan`: `OverrideSpeed(yy)`, `AddProfile(ss)`, `GetProfile(s)`, …
  - `com.tux.Tailor.Profiles`: `Reload`, `SetActiveProfileName(s)`, `GetActiveProfileName`,
    `GetNumberOfFans`, …
- **Failure mode:** with `performance_profile = "enthusiast"` saved into the active profile,
  the fan ran loud at a **cold 35 °C idle**, ignoring the 0 %-below-49 °C curve. Calling
  `Performance.SetProfile "power_save"` over D-Bus changed the reported profile but did
  **not** quiet the fan; the on-disk profile still read `enthusiast`. So on this
  Uniwill-AMD board the EC ignores tailord's fan duty, or couples it to the performance
  profile in a way that forces a fan floor. Phase 1 explains the cause via `strace` plus
  the driver source.

## Fan-curve JSON format (tailord, for reference)

```json
[ { "temp": 25, "fan": 0 }, { "temp": 49, "fan": 0 }, { "temp": 62, "fan": 24 },
  { "temp": 80, "fan": 60 }, { "temp": 87, "fan": 97 } ]
```
tailord warns if the last point isn't 100 % and clamps very-low duties (`"Fan speed 28% at
65°C is too low. Falling back to 30%"`). Replicate or override that behaviour.

## Temps / fan readout (no TUXEDO tool needed)

- CPU: `/sys/class/hwmon/hwmon*/temp1_input` where `name == k10temp`.
- Also `amdgpu`, `acpitz`, `nvme`, `spd5118` (RAM), `mt7925_phy0` (Wi-Fi).
- Fan RPM is **not** exposed via hwmon on this board; it comes through `tuxedo_io`
  (another reason the ioctl map matters). `lm_sensors` shows temps but not the fan.

## Open questions for Phase 1

1. Which `TUXEDO_IO_*` ioctl sets fan duty on the Uniwill path, and what's the arg encoding?
2. Does setting a performance profile write an EC register that forces a minimum fan duty
   independent of the requested duty? (Explains why `power_save` didn't help.)
3. Can fan duty be set to 0 % / auto on this board at all, or does the EC clamp it?
4. How does `tccd` differ from `tailord` in the exact ioctl sequence on this board?
