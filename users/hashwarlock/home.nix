{pkgs, ...}: {
  ##################################################################################################################
  #
  # All hashwarlock's Home Manager Configuration
  #
  ##################################################################################################################

  imports = [
    ../../home/core.nix

    ../../home/fcitx5
    ../../home/i3
    ../../home/programs
    ../../home/rofi
    ../../home/shell
  ];

  programs.git = {
    userName = "Joshua W";
    userEmail = "hashwarlock@protonmail.com";
  };
}
