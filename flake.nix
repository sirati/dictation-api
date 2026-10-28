{
  description = "Dictation broker, Rust SDK, and generated language bindings";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.nixpkgs-darwin-intel.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";
  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-darwin-intel,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      each = nixpkgs.lib.genAttrs systems;
      pkgsFor =
        system:
        import (if system == "x86_64-darwin" then nixpkgs-darwin-intel else nixpkgs) { inherit system; };
    in
    {
      overlays.default = final: prev: {
        dictation-broker = self.packages.${prev.stdenv.hostPlatform.system}.default;
      };
      homeManagerModules.default = { pkgs, lib, ... }: {
        imports = [ ./nix/home-manager.nix ];
        services.dictation-api.package =
          lib.mkDefault
            self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      };
      nixosModules.default = { pkgs, lib, ... }: {
        imports = [ ./nix/nixos.nix ];
        services.dictation-api.package =
          lib.mkDefault
            self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      };
      packages = each (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          node-addon = pkgs.callPackage ./nix/node.nix { };
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "dictation-broker";
            version = "0.1.0";
            src = pkgs.lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeCheckInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.dbus ];
            cargoBuildFlags = [
              "-p"
              "dictation-broker"
            ];
            cargoTestFlags = [
              "-p"
              "dictation-broker"
              "-p"
              "dictation-api"
              "-p"
              "dictation-protocol"
            ];
            meta.mainProgram = "dictation-broker";
          };
        }
      );
      devShells = each (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              pkg-config
              gcc
              python3
              maturin
              rust-cbindgen
              nodejs_26
              typescript
              nixfmt
              ripgrep
              jq
              uv
              just
              cargo-xwin
              clang
              lld
              llvmPackages.llvm
              rustup
            ];
            buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.dbus ];
          };
        }
        // pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          linux-test = pkgs.mkShell {
            inputsFrom = [ self.devShells.${system}.default ];
            packages = [
              (pkgs.python3.withPackages (ps: [ ps.pygobject3 ]))
              pkgs.sway
              pkgs.wayvnc
              pkgs.dbus
            ];
            buildInputs = [
              pkgs.gtk4
              pkgs.gobject-introspection
              pkgs.at-spi2-core
            ];
            DICTATION_TEST_ATSPI = "${pkgs.at-spi2-core}";
          };
        }
      );
      checks = each (
        system:
        let
          pkgs = pkgsFor system;
        in
        pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          service-modules = import ./nix/module-check.nix { inherit self pkgs nixpkgs; };
        }
      );
      formatter = each (system: (pkgsFor system).nixfmt);
    };
}
