# prober (Phase 2)

A tiny standalone tool to validate direct control of `/dev/tuxedo_io` before any daemon work:
read CPU/GPU temp + fan RPM, set fan duty to an explicit percent (and "auto").

Success criterion: drive the fan to 0% at a cold idle, then watch it ramp under load,
proving fan duty is controllable independent of the performance profile (the bug tailord
has on Uniwill-AMD; see ../docs/hardware-interface.md).

Not yet implemented; depends on the Phase 1 ioctl map.
