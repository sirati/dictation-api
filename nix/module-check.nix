{
  self,
  pkgs,
  nixpkgs,
}:
let
  lib = pkgs.lib;
  stub = {
    options = {
      home.packages = lib.mkOption {
        type = lib.types.listOf lib.types.package;
        default = [ ];
      };
      assertions = lib.mkOption {
        type = lib.types.listOf lib.types.unspecified;
        default = [ ];
      };
      systemd.user.services = lib.mkOption {
        type = lib.types.attrsOf lib.types.unspecified;
        default = { };
      };
      launchd.agents = lib.mkOption {
        type = lib.types.attrsOf lib.types.unspecified;
        default = { };
      };
    };
  };
  hm = lib.evalModules {
    specialArgs = { inherit pkgs; };
    modules = [
      stub
      self.homeManagerModules.default
      { services.dictation-api.enable = true; }
    ];
  };
  unit = hm.config.systemd.user.services.dictation-api;
  mac = lib.evalModules {
    specialArgs.pkgs = import nixpkgs { system = "aarch64-darwin"; };
    modules = [
      stub
      self.homeManagerModules.default
      { services.dictation-api.enable = true; }
    ];
  };
  agent = mac.config.launchd.agents.dictation-api.config;
  nixos = nixpkgs.lib.nixosSystem {
    system = pkgs.stdenv.hostPlatform.system;
    modules = [
      self.nixosModules.default
      {
        services.dictation-api.enable = true;
        system.stateVersion = "26.05";
      }
    ];
  };
  systemUnit = nixos.config.systemd.user.services.dictation-api;
in
assert unit.Install.WantedBy == [ "graphical-session.target" ];
assert
  unit.Service.ExecStart
  == "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/dictation-broker run";
assert unit.Service.UMask == "0077";
assert systemUnit.wantedBy == [ "graphical-session.target" ];
assert systemUnit.serviceConfig.ExecStart == unit.Service.ExecStart;
assert
  agent.ProgramArguments == [
    "${self.packages.aarch64-darwin.default}/bin/dictation-broker"
    "run"
  ];
assert agent.RunAtLoad && agent.Umask == 63;
assert agent.ProcessType == "Interactive";
pkgs.runCommand "dictation-service-module-check" { } ''
  mkdir -p "$out"
  cat > "$out/home-manager-unit.json" <<'JSON'
  ${builtins.toJSON unit}
  JSON
''
