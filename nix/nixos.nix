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
    enable = lib.mkEnableOption "the dictation broker in each graphical user session";
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.dictation-broker;
    };
    environment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
    };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    services.dbus.enable = true;
    systemd.user.services.dictation-api = {
      description = "Application dictation broker";
      after = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];
      environment = cfg.environment;
      serviceConfig = {
        ExecStart = "${cfg.package}/bin/dictation-broker run";
        Restart = "on-failure";
        RestartSec = 2;
        UMask = "0077";
      };
    };
  };
}
