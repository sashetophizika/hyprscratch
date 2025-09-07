self: {
  config,
  pkgs,
  lib,
  ...
}: let
  inherit (lib.modules) mkIf;
  inherit (lib.options) mkOption mkEnableOption literalExpression;
  inherit (lib.hm.generators) toHyprconf;
  inherit (lib.meta) getExe;
  inherit
    (lib.types)
    package
    either
    str
    attrsOf
    ;

  cfg = config.programs.hyprscratch;
in {
  options.programs.hyprscratch = {
    enable = mkEnableOption "hyprscratch";

    package = mkOption {
      type = package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = literalExpression ''
        hyprscratch.packages.''${pkgs.stdenv.hostPlatform.system}.default
      '';
      description = ''
        Hyprscratch package to use. Defaults to the one provided by the flake.
      '';
    };

    settings = mkOption {
      type = attrsOf (either str (attrsOf str));
      default = {};
      description = ''
        Hyprscratch configuration written in Nix.
      '';
      example = literalExpression ''
        {
          # Optional globals that apply to all scratchpads
          daemon_options = "clean";
          global_options = "special";
          global_rules = "size 90% 90%";

          name = {
              # Mandatory fields
              command = "command";

              # At least one is mandatory, title takes priority
              title = "title";
              class = "class";

              # Optional fields
              options = "option1 option2 option3";
              rules = "rule1;rule2;rule3";
          };
        }
      '';
    };
  };

  config = mkIf cfg.enable {
    home.packages = [cfg.package];

    systemd.user.services.hyprscratch = {
      Install.WantedBy = ["hyprland-session.target"];

      Unit = {
        Description = "Hyprscratch: Improved scratchpad functionality for Hyprland";
        PartOf = ["hyprland-session.target"];
        After = ["hyprland-session.target"];
      };

      Service = {
        Type = "simple";
        Restart = "on-failure";
        RestartSec = "5s";
        ExecStart = "${getExe cfg.package} init";
      };
    };

    xdg.configFile."hypr/hyprscratch.conf" = mkIf (cfg.settings != {}) {
      text = toHyprconf {attrs = cfg.settings;};
    };
  };
}
