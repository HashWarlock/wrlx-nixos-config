{...}: {
  # Install git via home-manager module
  programs.git = {
    enable = true;
    delta = {
      enable = true;
      catppuccin.enable = true;
      options = {
        keep-plus-minus-markers = true;
        light = false;
        line-numbers = true;
        navigate = true;
        width = 280;
      };
    };
    extraConfig = {
      pull.rebase = "true";
    };
  };
}
