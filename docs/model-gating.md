# Model gating: generalising beyond the Gen9 reference board

Status: design (source-derived from `tuxedo-drivers`). The reference board (TUXEDO
InfinityBook Pro AMD Gen9, Uniwill `GXxHRXx`) is the only **validated** board; this
document is the plan to run safely on other Uniwill boards without guessing.

## Safety rule (non-negotiable)

The daemon writes fan duty to the embedded controller. An unvalidated board may use a
different fan scaling, control path, or register layout. Therefore: **an unknown model
gets no fan/EC writes — the daemon runs read-only until the board is validated with the
prober and added to the known-models registry.** This mirrors `CONTRIBUTING.md`: "don't
change the default path for boards you can't test."

## What the driver actually gates on (source findings)

Derived from the GPL `tuxedo-drivers` (`src/tuxedo_io/tuxedo_io.c`,
`src/tuxedo_io/tuxedo_io_ioctl.h`, `src/uniwill_keyboard.h`, `src/uniwill_interfaces.h`).

- **Model id** (`R_UW_MODEL_ID`, ioctl nr `0x01`) returns `uw_feats->model`, read from EC
  RAM `0x0740` (barebone id). It is an enum of known board families
  (`uniwill_interfaces.h`), e.g. `PH4Txxx`/`PH6Txxx`/`PFxxxxx` variants. It is the handle
  the roadmap calls for, and what we key the registry on.

- **The real fan-path discriminator is NOT the model id directly** — it is
  `uw_feats->uniwill_has_universal_ec_fan_control`, auto-detected from **EC RAM `0x078e`
  bit 6** after applying a DMI/model exception list (`uniwill_keyboard.h`). Two paths:
  - **Universal EC fan control ("NB02", our reference board):** fan duty written to custom
    tables (`0x0f20` CPU / `0x0f50` GPU), max = `NB02_FAN_SPEED_MAX = 0xc8` (200), and the
    `0x0751` bit `0x40` must be **CLEAR** for custom control (this is exactly the fix our
    port applies).
  - **Legacy direct path:** fan duty written directly (`0x1804`/`0x1809`), different max,
    and `0x0751` bit `0x40` is **SET** for active control — i.e. the *opposite* meaning.
  Our port's entire fan-write contract (`0xc8` scale + clear `0x40`) is therefore the
  **universal path only**. Boards on the legacy path are out of scope until validated.

- **Performance profiles** (`W_UW_PERF_PROF`, nr `0x18`): ids `1/2/3` =
  PowerSave/Enthusiast/Overboost, writing bits `0xa0`/`0x00`/`0x10` into EC RAM `0x0751`
  (clearing `0xa0|0x10` first). Support is model-gated; the count is reported by
  `R_UW_PROFS_AVAILABLE` (nr `0x21`) → `0`, `2`, or `3`.

- **Capability ioctls (uniform vs gated):**
  | ioctl | nr | driver returns | model-gated? |
  |---|---|---|---|
  | `R_UW_FANS_OFF_AVAILABLE` | `0x16` | hardcoded `1` | no (uniform) |
  | `R_UW_FANS_MIN_SPEED` | `0x17` | hardcoded `25` (`FAN_ON_MIN_SPEED_PERCENT`) | no (uniform) |
  | `R_UW_PROFS_AVAILABLE` | `0x21` | `0` / `2` / `3` | **yes** |

## Design for our port

1. **`tuxedoio` crate (the safety boundary):** `TuxedoIo::open()` reads the model id and
   sets a `write_allowed` flag from the registry; `wr()`/`noarg()` (and therefore every EC
   write — `set_fan_pct`, `set_perf`, `ensure_manual`, `restore_auto`) refuse with
   `PermissionDenied` when the board is unvalidated. Reads are always allowed. The prober
   opens with `open_unchecked()` to validate a new board. This gate is enforced once, in the
   library, so every consumer (daemon, TUI) inherits it. Also adds `model_id()` and
   capability reads (`fans_off_available()`, `fans_min_speed()`, `profs_available()`) + a
   `Caps` struct. `FAN_MAX` stays `0xc8` (reference board); a future board with a different
   raw max requires making the scaling per-model before it is added to the registry.

2. **Known-models registry** keyed by `model_id`: each entry records the validated board,
   its `fan_max`, and notes. Only boards exercised on real hardware appear here.

3. **Daemon gating:** at startup read `model_id` + caps; log them. If the model is in the
   registry → full control with its `fan_max`. If **not** → refuse fan/EC writes
   (read-only mode), keep serving `STATUS` so the GUI/TUI can still show temps and report
   "unsupported board". Surface `model_id` and a validated/unknown flag in the `STATUS`
   JSON.

4. **Capability-driven behaviour:** use `R_UW_PROFS_AVAILABLE` to gate which performance
   profiles are offered, `R_UW_FANS_MIN_SPEED` for the on-speed floor (instead of the
   hardcoded 25%), and `R_UW_FANS_OFF_AVAILABLE` for whether 0% is allowed — detect at
   runtime, hide what the EC doesn't expose.

## Adding a board (process)

Per `CONTRIBUTING.md`: open an issue with hardware details + `tuxedo-prober info`, read the
driver source for this board's path, validate reads then writes with the prober (with
`auto` as the bail-out), add the model id to the registry, document here, and update the
[supported-hardware table](../README.md#supported-hardware). A board is supported only once
fan control is validated on it.

## Reference board (validated, from `tuxedo-prober info`)

- Board: InfinityBook Pro AMD Gen9, `GXxHRXx`, `tuxedo_io` 0.3.9.
- **`model id` = `0x1a`** · `fans-off available` = `1` · `fans min speed %` = `25` ·
  `perf profiles avail` = `3` · mode `0x0751` = `0x00` (bit `0x40` clear → universal path,
  manual fan control OK).
- `fan_max` = `0xc8` (universal-EC-fan-control path). This is the single entry that seeds
  the known-models registry. Note `0x1a` is outside the model enum published in the
  `tuxedo-drivers` snapshot we read, so the live read is the source of truth here.
