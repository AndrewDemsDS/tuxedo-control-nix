# Phase 1: tuxedo_io ioctl protocol map + root-cause of the fan bug

Source-derived from `tuxedo-drivers` (`src/tuxedo_io/tuxedo_io_ioctl.h`,
`tuxedo_io.c`, `src/uniwill_keyboard.h`). Reference board: InfinityBook Pro AMD Gen9
(Uniwill, `uniwill_has_universal_ec_fan_control` = the "NB02" path).

## ioctl numbers

`IOCTL_MAGIC = 0xEC`. The argument is always a pointer to `int32_t` (read or write),
except `W_UW_FANAUTO` which takes no argument.

| Name | Encoding | Dir | Meaning |
|---|---|---|---|
| `R_MOD_VERSION`   | `_IOR(0xEC, 0x00, char*)` | R | module version string |
| `R_HWCHECK_UW`    | `_IOR(0xEC, 0x06, int*)`  | R | 1 if Uniwill hardware |
| `R_UW_MODEL_ID`   | `_IOR(0xEF, 0x01, int*)`  | R | model id |
| `R_UW_FANSPEED`   | `_IOR(0xEF, 0x10, int*)`  | R | CPU fan duty (raw, 0..0xc8) |
| `R_UW_FANSPEED2`  | `_IOR(0xEF, 0x11, int*)`  | R | GPU fan duty (raw) |
| `R_UW_FAN_TEMP`   | `_IOR(0xEF, 0x12, int*)`  | R | CPU fan temp °C |
| `R_UW_FAN_TEMP2`  | `_IOR(0xEF, 0x13, int*)`  | R | GPU fan temp °C |
| `R_UW_MODE`       | `_IOR(0xEF, 0x14, int*)`  | R | **reads EC RAM 0x0751** (the mode/perf/fan-control byte) |
| `R_UW_FANS_OFF_AVAILABLE` | `_IOR(0xEF, 0x16, int*)` | R | fans-off supported |
| `R_UW_FANS_MIN_SPEED`     | `_IOR(0xEF, 0x17, int*)` | R | min on-speed percent (25) |
| `R_UW_PROFS_AVAILABLE`    | `_IOR(0xEF, 0x21, int*)` | R | number of perf profiles |
| `W_UW_FANSPEED`   | `_IOW(0xF0, 0x10, int*)`  | W | set CPU fan duty (0..0xc8) |
| `W_UW_FANSPEED2`  | `_IOW(0xF0, 0x11, int*)`  | W | set GPU fan duty |
| `W_UW_MODE`       | `_IOW(0xF0, 0x12, int*)`  | W | **writes EC RAM 0x0751** (`arg & 0xff`) |
| `W_UW_FANAUTO`    | `_IO (0xF0, 0x14)`        | W | restore EC automatic fan control |
| `W_UW_PERF_PROF`  | `_IOW(0xF0, 0x18, int*)`  | W | set perf profile (1/2/3) |

`MAGIC_READ_UW = 0xEC+3 = 0xEF`, `MAGIC_WRITE_UW = 0xEC+4 = 0xF0`.

## Fan duty scaling (this board / NB02)

- `NB02_FAN_SPEED_MAX = 0xc8` (200) = 100 %. So `raw = round(percent * 200 / 100)`.
- `uw_set_fan` clamps to a min on-speed band: below `25%*200/2/100 = 25` → forced to `0`;
  below `25%*200/100 = 50` → forced to `50` (i.e. **on-speed floor ≈ 25 %**, off below ~12.5 %).
- Writing `0` is remapped to `1` internally (raw 0 makes the EC spin to 30 % for 3 min first;
  raw 1 = real off on 2020+ fans). So "fan off" = write `0`, driver sends `1`.

## Root cause of the loud-fan bug (why this project exists)

`uw_set_fan()` (`src/uniwill_keyboard.h`):

```c
if (uw_feats->uniwill_has_universal_ec_fan_control) {
    uniwill_read_ec_ram(0x0751, &byte_data);
    if (!(byte_data & 0x40)) {          // <-- only when bit 0x40 is CLEAR
        uw_init_fan();
        ... write custom fan table 0x0f20/0x0f50, direct_fan_control() ...
    }
    // else: REQUEST SILENTLY IGNORED -> EC keeps its own (loud) auto curve
}
```

So **manual fan duty only takes effect when bit `0x40` of EC RAM `0x0751` is clear.** If
`0x40` is set, every `W_UW_FANSPEED` is a no-op and the fan runs on the EC's auto curve.

`0x0751` is the same byte the perf profile (`uw_set_performance_profile_v1`, bits `0xb0`)
and `W_UW_MODE` write. On the reference machine, after cycling profiles in tailor_gui the
fan stayed loud at 35 °C and no `W_UW_FANSPEED`/`SetProfile` quieted it. That matches
`0x40` being left set, so the EC dropped tailord's fan writes.

### The fix (validated in Phase 2)

Before asserting fan duty: read `0x0751` (`R_UW_MODE`), and if bit `0x40` is set, clear it
(`W_UW_MODE` with `value & ~0x40`). Then `W_UW_FANSPEED` is honoured. The daemon also
re-asserts the duty each tick so a stray profile/mode write can't wedge it again. Keep the
perf profile a *separate*, explicit control; never let it silently flip fan ownership.

## Perf profile register `0x0751`

`uw_set_performance_profile_v1`: `clear_bits = 0xa0|0x10`; then
`POWERSAVE(1) |= 0xa0`, `ENTHUSIAST(2) |= 0x00`, `OVERBOOST(3) |= 0x10`. Bit `0x40` is
*not* owned by this function. It's the fan-control-ownership bit, toggled elsewhere
(`W_UW_MODE` / `set_full_fan_mode` / EC firmware), so a stuck `0x40` breaks fan control
silently, independent of the chosen profile.
