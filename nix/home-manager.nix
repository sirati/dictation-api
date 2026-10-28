{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.dictation-api;
in
{
  options.services.dictation-api = {
    enable = lib.mkEnableOption "the per-user dictation broker";
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.dictation-broker;
      description = "Dictation broker package.";
    };
    environment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      description = "Additional broker environment variables.";
    };
  };
  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = pkgs.stdenv.hostPlatform.isLinux;
        message = "services.dictation-api requires a Linux systemd user session.";
      }
    ];
    home.packages = [ cfg.package ];
    systemd.user.services.dictation-api = {
      Unit = {
        Description = "Application dictation broker";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Install.WantedBy = [ "graphical-session.target" ];
      Service = {
        ExecStart = "${cfg.package}/bin/dictation-broker run";
        Restart = "on-failure";
        RestartSec = 2;
        UMask = "0077";
        Environment = lib.mapAttrsToList (
          name: value: builtins.toJSON (lib.replaceStrings [ "%" ] [ "%%" ] "${name}=${value}")
        ) cfg.environment;
      };
    };
  };
}
