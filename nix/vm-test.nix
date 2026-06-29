# NixOS VM test: boots a machine with services.tuxedo-control enabled and verifies the
# module end-to-end. The test VM has no TUXEDO embedded controller, so we don't assert the
# daemon stays running (it exits when /dev/tuxedo_io is absent). Instead we verify the parts
# that don't need hardware: the systemd unit is generated and points at the daemon binary,
# the conflicting tuxedo-rs/tailord daemon is force-disabled, the declarative options reach
# the generated config.json, and the daemon binary runs far enough to fail *gracefully*
# (non-zero exit, no panic) on the missing device.
{ pkgs, nixosModule }:
pkgs.testers.runNixOSTest {
  name = "tuxedo-control";

  nodes.machine = { lib, ... }: {
    imports = [ nixosModule ];

    # No TUXEDO EC in the test VM: don't build/load the out-of-tree drivers, and drop the
    # tuxedo_io kernel module the module would otherwise queue (it can't load here, and a
    # failed systemd-modules-load would only add noise).
    hardware.tuxedo-drivers.enable = lib.mkForce false;
    boot.kernelModules = lib.mkForce [ ];

    services.tuxedo-control = {
      enable = true;
      performanceProfile = "power_save";
      keyboard.backlight.brightness = 2;
      charging.profile = "stationary";
      fan.curve = [
        {
          temp = 30;
          speed = 0;
        }
        {
          temp = 80;
          speed = 60;
        }
      ];
    };
  };

  testScript = ''
    machine.wait_for_unit("multi-user.target")

    # Our service unit is installed and wired to the daemon binary + generated config.
    execstart = machine.succeed(
        "systemctl cat tuxedo-control.service | sed -n 's/^ExecStart=//p'"
    ).strip()
    parts = execstart.split()
    assert len(parts) == 2, f"unexpected ExecStart: {execstart!r}"
    daemon, config_path = parts
    assert daemon.endswith("/tuxedo-controld"), daemon

    # The conflicting upstream fan daemon must be force-disabled (no unit installed).
    machine.fail("systemctl cat tailord.service")

    # The declarative options must reach the generated config.json.
    config = machine.succeed(f"cat {config_path}")
    assert '"kbd_brightness":2' in config, config
    assert '"charge_profile":"stationary"' in config, config
    assert '"perf_profile":"power_save"' in config, config

    # The daemon binary runs and fails *gracefully* (non-zero exit, no panic) when
    # /dev/tuxedo_io is absent, as it is in the hardware-less test VM.
    out = machine.fail(f"{daemon} {config_path} 2>&1")
    assert "tuxedo_io" in out, out
  '';
}
