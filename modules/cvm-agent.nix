# CVM Agent NixOS Module
# Provides systemd service definitions and configuration options for the CVM Agent

{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.cvm-agent;
in
{
  options.services.cvm-agent = {
    enable = mkEnableOption "CVM Agent service";

    port = mkOption {
      type = types.port;
      default = 8080;
      description = "Port for the agent API";
    };

    webPort = mkOption {
      type = types.port;
      default = 8081;
      description = "Port for the web UI";
    };

    environment = mkOption {
      type = types.attrsOf types.str;
      default = {};
      description = "Environment variables for the agent";
      example = literalExpression ''
        {
          REDPILL_API_KEY = "your-key";
          REDPILL_MODEL = "claude-3-5-sonnet-20241022";
        }
      '';
    };

    workingDirectory = mkOption {
      type = types.path;
      default = "/opt/cvm-agent";
      description = "Working directory for the CVM Agent";
    };

    user = mkOption {
      type = types.str;
      default = "cvm-agent";
      description = "User account under which the CVM Agent runs";
    };

    group = mkOption {
      type = types.str;
      default = "cvm-agent";
      description = "Group account under which the CVM Agent runs";
    };
  };

  config = mkIf cfg.enable {
    # Create user and group for the service
    users.users.${cfg.user} = mkIf (cfg.user == "cvm-agent") {
      isSystemUser = true;
      group = cfg.group;
      home = cfg.workingDirectory;
      description = "CVM Agent service user";
    };

    users.groups.${cfg.group} = mkIf (cfg.group == "cvm-agent") {};

    # Agent API service
    systemd.services.cvm-agent = {
      description = "CVM Agent API";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];

      environment = {
        RUST_LOG = "info";
        CVM_AGENT_PORT = toString cfg.port;
        CVM_AGENT_WEB_PORT = toString cfg.webPort;
      } // cfg.environment;

      serviceConfig = {
        Type = "simple";
        User = cfg.user;
        Group = cfg.group;
        WorkingDirectory = cfg.workingDirectory;
        # TODO: Replace with actual package path after building
        ExecStart = "${cfg.workingDirectory}/agent-api";
        Restart = "always";
        RestartSec = "5s";

        # Security hardening
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        ReadWritePaths = [ cfg.workingDirectory ];
      };
    };

    # Open firewall ports (only if firewall is enabled)
    networking.firewall.allowedTCPPorts = mkIf config.networking.firewall.enable [
      cfg.port
      cfg.webPort
    ];
  };
}
