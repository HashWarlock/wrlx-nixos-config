{...}: {
  ##################################################################################################################
  #
  # Confidant User - Home Manager Configuration (Lightweight for CVM)
  #
  ##################################################################################################################

  imports = [
    # Minimal common modules for terminal use
    ../../home/modules/git.nix
    ../../home/modules/zsh.nix
    ../../home/modules/tmux.nix
    ../../home/modules/lazygit.nix
    ../../home/modules/bat.nix
    ../../home/modules/fzf.nix
    ../../home/modules/bottom.nix
    ../../home/modules/fastfetch.nix
  ];

  # Enable home-manager
  programs.home-manager.enable = true;

  # State version
  home.stateVersion = "25.11";

  # Git configuration for CVM user
  programs.git = {
    userName = "Confidant";
    userEmail = "confidant@phala-cvm";
  };

  # Catpuccin theming (lightweight)
  catppuccin = {
    flavor = "macchiato";
    accent = "lavender";
  };
}
