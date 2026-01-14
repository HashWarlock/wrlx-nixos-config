{ pkgs ? import <nixpkgs> { }
, lib ? pkgs.lib
}:

# Nix package for CVM Desktop (Tauri application)
# This can be built with: nix-build -E '(import <nixpkgs> {}).callPackage ./default.nix {}'
# Or used in a flake: packages.cvm-desktop = pkgs.callPackage ./cvm-agent/desktop { };

pkgs.stdenv.mkDerivation rec {
  pname = "cvm-desktop";
  version = "0.1.0";
  src = ./.;

  nativeBuildInputs = with pkgs; [
    # Rust toolchain
    rustc
    cargo
    pkg-config

    # Build tools
    nodejs
    nodePackages.npm

    # Protobuf for gRPC
    protobuf
  ];

  buildInputs = with pkgs; [
    # Tauri WebView dependencies
    webkitgtk_4_1
    gtk3
    libsoup_3
    glib
    glib-networking
    gsettings-desktop-schemas

    # System tray support
    libayatana-appindicator
    at-spi2-core

    # TLS/crypto
    openssl
  ];

  # Environment for Rust builds
  OPENSSL_DIR = "${pkgs.openssl.dev}";
  OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
  OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

  buildPhase = ''
    # Install npm dependencies
    export HOME=$(mktemp -d)
    npm install --prefer-offline

    # Build frontend
    npm run build

    # Build Tauri app
    cd src-tauri
    cargo build --release
    cd ..
  '';

  installPhase = ''
    mkdir -p $out/bin
    cp src-tauri/target/release/cvm-desktop $out/bin/

    # Copy icon for desktop integration
    mkdir -p $out/share/icons/hicolor/256x256/apps
    cp src-tauri/icons/icon.png $out/share/icons/hicolor/256x256/apps/cvm-desktop.png

    # Create desktop entry
    mkdir -p $out/share/applications
    cat > $out/share/applications/cvm-desktop.desktop <<EOF
    [Desktop Entry]
    Name=CVM Agent
    Comment=CVM Agent Desktop Application
    Exec=$out/bin/cvm-desktop
    Icon=cvm-desktop
    Terminal=false
    Type=Application
    Categories=Utility;
    StartupWMClass=cvm-desktop
    EOF
  '';

  meta = with lib; {
    description = "CVM Agent Desktop Application (Tauri)";
    homepage = "https://github.com/wrlx/nixos-config";
    license = licenses.mit;
    platforms = platforms.linux;
    mainProgram = "cvm-desktop";
  };
}
