{
  outputs,
  pkgs,
  lib,
  username,
  ...
}: {
  # ============================= CVM System Configuration =============================
  # Minimal system configuration for Confidential VM environments
  # No hardware-specific dependencies

  # Define user account
  users.users.${username} = {
    isNormalUser = true;
    description = username;
    extraGroups = ["networkmanager" "docker" "wheel"];
  };

  # Trusted users for nix
  nix.settings.trusted-users = [username];

  # Nix configuration
  nix.settings = {
    experimental-features = ["nix-command" "flakes"];
    auto-optimise-store = true;
  };

  # Garbage collection
  nix.gc = {
    automatic = lib.mkDefault true;
    dates = lib.mkDefault "weekly";
    options = lib.mkDefault "--delete-older-than 365d";
  };

  # Allow unfree packages and enable overlay
  nixpkgs = {
    overlays = [
      outputs.overlays.unstable-packages
    ];
    config.allowUnfree = true;
  };

  # Timezone
  time.timeZone = "America/Chicago";

  # Locale
  i18n.defaultLocale = "en_US.UTF-8";
  i18n.extraLocaleSettings = {
    LC_ADDRESS = "en_US.UTF-8";
    LC_IDENTIFICATION = "en_US.UTF-8";
    LC_MEASUREMENT = "en_US.UTF-8";
    LC_MONETARY = "en_US.UTF-8";
    LC_NAME = "en_US.UTF-8";
    LC_NUMERIC = "en_US.UTF-8";
    LC_PAPER = "en_US.UTF-8";
    LC_TELEPHONE = "en_US.UTF-8";
    LC_TIME = "en_US.UTF-8";
  };

  # Disable printing (not needed in CVM)
  services.printing.enable = false;

  # Security
  security.rtkit.enable = true;

  # Minimal fonts for GUI
  fonts.packages = with pkgs; [
    (nerdfonts.override {fonts = ["JetBrainsMono"];})
  ];

  # Enable zsh
  programs.zsh.enable = true;

  # Disable firewall for CVM (handled at container level)
  networking.firewall.enable = false;

  # Docker configuration
  virtualisation.docker.enable = true;
  virtualisation.docker.rootless.enable = true;
  virtualisation.docker.rootless.setSocketVariable = true;

  # Sound (minimal, may not work in container)
  sound.enable = false;
  hardware.pulseaudio.enable = false;

  # Essential system packages
  environment.systemPackages = with pkgs; [
    python3
    helix
    wget
    curl
    gcc
    git
    jq
    docker-compose
  ];
}
