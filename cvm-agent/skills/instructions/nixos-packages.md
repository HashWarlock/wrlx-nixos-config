---
name: nixos-packages
description: Knowledge about NixOS package management, overlays, and overrides
triggers:
  - package
  - packages
  - install package
  - overlay
  - overlays
  - override
  - pkgs.unstable
  - unfree
---

# NixOS Package Management Knowledge

## Adding Packages

### System-wide packages (NixOS)
In `configuration.nix` or a module:

```nix
environment.systemPackages = with pkgs; [
  vim
  git
  htop
];
```

### User packages (Home Manager)
In home-manager config:

```nix
home.packages = with pkgs; [
  firefox
  vscode
  spotify
];
```

## Overlays

Overlays modify or add packages. Define in `overlays/default.nix`:

```nix
final: prev: {
  # Add unstable packages as pkgs.unstable.*
  unstable = import inputs.nixpkgs-unstable {
    system = prev.system;
    config.allowUnfree = true;
  };

  # Override existing package
  vim = prev.vim.override {
    python3 = prev.python311;
  };

  # Add new package
  my-script = prev.writeShellScriptBin "my-script" ''
    echo "Hello from my script!"
  '';
}
```

Apply overlay in flake:

```nix
nixpkgs.overlays = [ (import ./overlays) ];
```

## Package Overrides

### Simple override (change inputs)
```nix
pkgs.package.override {
  enableFeature = true;
  dependency = pkgs.other-dependency;
}
```

### Deep override (change derivation)
```nix
pkgs.package.overrideAttrs (old: {
  version = "2.0";
  src = pkgs.fetchFromGitHub { ... };
  patches = old.patches ++ [ ./my-fix.patch ];
})
```

## Unfree Packages

Enable unfree packages:

```nix
nixpkgs.config.allowUnfree = true;
```

Or for specific packages:

```nix
nixpkgs.config.allowUnfreePredicate = pkg:
  builtins.elem (lib.getName pkg) [
    "vscode"
    "spotify"
    "slack"
  ];
```

## Finding Packages

### Search online
- https://search.nixos.org/packages

### Search from CLI
```bash
nix search nixpkgs firefox
nix search nixpkgs-unstable vscode
```

### List installed packages
```bash
nix-env -q  # legacy
```

## Common Patterns

### Pin to unstable version
```nix
environment.systemPackages = [
  pkgs.unstable.neovim  # Use unstable overlay
];
```

### FHS-compatible packages
Some programs need traditional Linux filesystem:

```nix
pkgs.buildFHSUserEnv {
  name = "fhs-shell";
  targetPkgs = pkgs: with pkgs; [
    gcc
    binutils
  ];
}
```

Or use `-fhs` variants: `pkgs.vscode-fhs`

### Wrap with extra deps
```nix
pkgs.symlinkJoin {
  name = "my-wrapped-app";
  paths = [ pkgs.app ];
  buildInputs = [ pkgs.makeWrapper ];
  postBuild = ''
    wrapProgram $out/bin/app \
      --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.git ]}
  '';
}
```
