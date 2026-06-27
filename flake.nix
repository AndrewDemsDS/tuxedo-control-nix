{
  description = "tuxedo-control-nix: declarative TUXEDO Uniwill fan/performance control via the tuxedo_io ioctl interface";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  # (Optional) once a Cachix cache exists, add it here:
  #   nixConfig.extra-substituters = [ "https://<cache>.cachix.org" ];
  #   nixConfig.extra-trusted-public-keys = [ "<cache>.cachix.org-1:<real-base64-key>" ];

  outputs = { self, nixpkgs, flake-utils }:
    let
      # NixOS module (system-agnostic). Passes `self` so it can default to this flake's package.
      nixosModule = import ./nix/module.nix self;
    in
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = import nixpkgs { inherit system; };
      in {
        packages.default = pkgs.callPackage ./nix/package.nix { };
        packages.tuxedo-control = self.packages.${system}.default;

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/tuxedo-gui";
        };

        formatter = pkgs.nixfmt-rfc-style;

        # `nix flake check` builds these. The package build covers compile + cargo test
        # (buildRustPackage checkPhase); fmt/clippy run in CI inside the dev shell.
        checks = {
          build = self.packages.${system}.default;
          # Cheap end-to-end module eval: forcing ExecStart evaluates the whole module;
          # a broken option/type/assertion fails the check without a VM build.
          module-eval = pkgs.runCommand "module-eval"
            {
              execStart = (nixpkgs.lib.nixosSystem {
                inherit system;
                modules = [
                  nixosModule
                  ({ ... }: {
                    boot.loader.grub.enable = false;
                    fileSystems."/" = { device = "/dev/sda1"; fsType = "ext4"; };
                    system.stateVersion = "25.05";
                    services.tuxedo-control.enable = true;
                  })
                ];
              }).config.systemd.services.tuxedo-control.serviceConfig.ExecStart;
            } ''printf '%s\n' "$execStart" > $out'';
        };

        devShells.default = pkgs.mkShell {
          # GTK stack so `cargo clippy`/`cargo test` can compile the gui crate in CI + locally.
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = with pkgs; [ gtk4 libadwaita glib ];
          packages = with pkgs; [
            rustc cargo rust-analyzer clippy rustfmt
            strace ltrace ripgrep jq
          ];
          shellHook = ''
            echo "tuxedo-control-nix dev shell"
            echo "  /dev/tuxedo_io: $( [ -e /dev/tuxedo_io ] && echo present || echo MISSING )"
            echo "  build: cargo build --release   |   probe: sudo ./target/release/tuxedo-prober info"
          '';
        };
      })
    // {
      overlays.default = final: _prev: {
        tuxedo-control = final.callPackage ./nix/package.nix { };
      };
      nixosModules.default = nixosModule;
      nixosModules.tuxedo-control = nixosModule;
    };
}
