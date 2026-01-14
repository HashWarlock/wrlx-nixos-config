{
  description = "hashwarlock NixOS flake";

  # the nixConfig here only affects the flake itself, not the system configuration!
  nixConfig = {
    # substituers will be appended to the default substituters when fetching packages
    # nix com    extra-substituters = [munity's cache server
    extra-substituters = [
      "https://nix-community.cachix.org"
    ];
    extra-trusted-public-keys = [
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  inputs = {
    # NixOS official package source, using the nixos-25.11 branch
    nixpkgs.url = "git+https://github.com/NixOS/nixpkgs?ref=nixos-25.11";
    nixpkgs-unstable.url = "git+https://github.com/nixos/nixpkgs?ref=nixos-unstable";
    # home-manager, used for managing user configuration
    home-manager = {
      url = "git+https://github.com/nix-community/home-manager?ref=release-25.11";
      # The `follows` keyword in inputs is used for inheritance.
      # Here, `inputs.nixpkgs` of home-manager is kept consistent with
      # the `inputs.nixpkgs` of the current flake,
      # to avoid problems caused by different versions of nixpkgs.
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Global catppuccin theme
    catppuccin.url = "git+https://github.com/catppuccin/nix";

    # NixOS Spicetify
    spicetify-nix = {
      url = "git+https://github.com/Gerg-L/spicetify-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

  };

  outputs = {
    self,
    catppuccin,
    nixpkgs,
    home-manager,
     ...
  } @ inputs: let
    # Package set for CVM development shell
    cvmPkgs = import nixpkgs { system = "x86_64-linux"; };
  in {
    # Please replace my-nixos with your hostname
    nixosConfigurations = {
      asus-g512lw = let
        username = "hashwarlock";
        specialArgs = {
          inherit username;
          inherit (self) outputs;
        };
      in
        nixpkgs.lib.nixosSystem {
        inherit specialArgs;
        system = "x86_64-linux";
        modules = [
          # Import the previous configuration.nix we used,
          # so the old configuration file still takes effect
          ./hosts/asus-g512lw
          ./users/${username}/nixos.nix
          catppuccin.nixosModules.catppuccin
          # make home-manager as a module of nixos
          # so that home-manager configuration will be deployed automatically when executing `nixos-rebuild switch`
          home-manager.nixosModules.home-manager
          {
            home-manager.useGlobalPkgs = false;
            home-manager.useUserPackages = true;

            home-manager.extraSpecialArgs = inputs // specialArgs;
            home-manager.users.${username} = {
              imports = [
                ./users/${username}/home.nix
                catppuccin.homeManagerModules.catppuccin
              ];
            };
          }
        ];
      };

      # Note: phala-cvm nixosConfiguration removed - using Docker + devshell approach
      # See files/phala-cvm/ for Docker-based deployment
    };
    overlays = import ./overlays {inherit inputs;};

    # Development shell for CVM with all required packages from flake.lock
    devShells.x86_64-linux.cvm = cvmPkgs.mkShell {
      packages = with cvmPkgs; [
        # VNC/GUI
        tigervnc
        xorg.xorgserver
        xfce.xfce4-session
        xfce.xfce4-panel
        xfce.xfwm4
        xfce.xfdesktop
        xfce.xfce4-settings
        xfce.xfce4-terminal
        xfce.xfce4-appfinder
        xfce.xfconf
        xfce.thunar
        novnc

        # System tools
        dbus
        openssh
        procps
        which
        bash
        hostname
        fontconfig
        coreutils
        gnused

        # GUI automation
        xdotool
        scrot
        at-spi2-core
        python3Packages.pyatspi

        # Voice transcription
        whisper-cpp

        # Rust toolchain (1.91.1 from nixos-25.11)
        rustc
        cargo
        gcc
        pkg-config
        openssl
        openssl.dev  # Development headers for Rust openssl crate
        protobuf  # Required for gRPC/protobuf compilation

        # Tauri native app dependencies
        webkitgtk_4_1  # WebView for Tauri (GTK4-based)
        gtk3
        libsoup_3
        glib
        glib-networking  # TLS support for libsoup
        gsettings-desktop-schemas  # Required for GIO
        libayatana-appindicator  # System tray support
        gdk-pixbuf  # Image loading for GTK
        hicolor-icon-theme  # Base icon theme

        # Python for scripts
        python3

        # Node.js for Tauri frontend build
        nodejs
        nodePackages.npm

        # Browser for in-VM browsing (user-facing, not required for agent)
        firefox

        # Better screenshot tool (provides 'import' command)
        imagemagick
      ];

      # Set up environment for Rust openssl crate and GUI dependencies
      shellHook = ''
        # OpenSSL for Rust crates
        export OPENSSL_DIR="${cvmPkgs.openssl.dev}"
        export OPENSSL_LIB_DIR="${cvmPkgs.openssl.out}/lib"
        export OPENSSL_INCLUDE_DIR="${cvmPkgs.openssl.dev}/include"
        export PKG_CONFIG_PATH="${cvmPkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"

        # Fontconfig - required for GTK/Tauri font rendering
        export FONTCONFIG_FILE="${cvmPkgs.fontconfig.out}/etc/fonts/fonts.conf"

        # GTK/GDK environment for Tauri desktop app
        export GDK_PIXBUF_MODULE_FILE="${cvmPkgs.gdk-pixbuf}/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
        export GSETTINGS_SCHEMA_DIR="${cvmPkgs.gsettings-desktop-schemas}/share/glib-2.0/schemas:${cvmPkgs.gtk3}/share/glib-2.0/schemas"
        export GIO_EXTRA_MODULES="${cvmPkgs.glib-networking}/lib/gio/modules"

        # XDG directories for desktop environment
        export XDG_DATA_DIRS="${cvmPkgs.gsettings-desktop-schemas}/share:${cvmPkgs.gtk3}/share:${cvmPkgs.hicolor-icon-theme}/share:''${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
      '';
    };
  };
}
