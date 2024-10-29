{...}: {
  ##################################################################################################################
  #
  # All hashwarlock's Home Manager Configuration
  #
  ##################################################################################################################

  imports = [
    #../../home/core.nix

    ../../home/modules/common.nix
    ../../home/modules/easyeffects.nix
    ../../home/modules/hyprland.nix
    ../../home/modules/gnome.nix
    ../../home/modules/ulauncher.nix
  ];

  # Enable home-manager
  programs.home-manager.enable = true;

  # https://nixos.wiki/wiki/FAQ/When_do_I_update_stateVersion
  home.stateVersion = "24.05";

  programs.git = {
    userName = "Joshua W";
    userEmail = "hashwarlock@protonmail.com";
  };
}
