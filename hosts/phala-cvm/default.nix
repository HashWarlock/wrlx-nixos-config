# Phala Cloud Confidential VM Configuration
# This configuration is designed to run in a CVM environment without hardware-specific dependencies

{ pkgs, ... }:

{
  imports = [
    ../../modules/cvm-system.nix
    ../../modules/cvm-agent.nix
  ];

  # No bootloader needed for containerized environment
  # boot.loader configuration is omitted

  # Network configuration
  networking.hostName = "phala-cvm";
  networking.networkmanager.enable = true;

  # Enable X11 and XFCE for lightweight GUI
  services.xserver = {
    enable = true;
    desktopManager.xfce.enable = true;
    # No display manager needed - will use VNC
    displayManager.startx.enable = true;
  };

  # VNC and remote access packages
  environment.systemPackages = with pkgs; [
    # VNC server
    tigervnc

    # noVNC for web-based access
    unstable.novnc

    # X virtual framebuffer
    xorg.xorgserver
    xvfb-run

    # Lightweight desktop utilities
    xfce.thunar
    xfce.xfce4-terminal

    # Web browser for GUI testing
    firefox

    # Essential tools
    vim
    wget
    curl
    git
    htop
  ];

  # Enable SSH daemon
  services.openssh = {
    enable = true;
    settings = {
      PermitRootLogin = "no";
      PasswordAuthentication = false; # Key-based auth only
    };
    openFirewall = true;
  };

  # Enable CVM Agent service
  services.cvm-agent = {
    enable = true;
    # Default ports: 8080 (API), 8081 (Web UI)
    # Environment variables can be configured here:
    # environment = {
    #   REDPILL_API_KEY = "your-key";
    #   REDPILL_MODEL = "claude-3-5-sonnet-20241022";
    # };
  };

  # System state version
  system.stateVersion = "25.11";
}
