# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This is a personal NixOS configuration using flakes and home-manager. The configuration manages both system-level (NixOS) and user-level (home-manager) settings in a declarative manner.

## Reference Documentation

**Primary reference for NixOS configuration best practices:**
https://nixos-and-flakes.thiscute.world/best-practices/intro

## Architecture

### Flake-based Configuration

This repository uses NixOS flakes with the following key inputs:
- `nixpkgs` (24.05 stable)
- `nixpkgs-unstable` (for bleeding-edge packages via overlay)
- `home-manager` (release-24.05)
- `catppuccin` (global theming)
- `spicetify-nix` (Spotify theming)

### Directory Structure

```
├── flake.nix              # Main flake configuration, defines nixosConfigurations
├── hosts/                 # Host-specific configurations
│   └── asus-g512lw/       # Current host configuration
│       ├── default.nix    # Host entry point, imports system modules
│       └── hardware-configuration.nix
├── modules/               # System-level NixOS modules
│   ├── system.nix         # Core system configuration (users, nix settings, packages)
│   ├── hyprland.nix       # Hyprland window manager config
│   ├── gnome.nix          # GNOME desktop config
│   └── i3.nix             # i3 window manager config
├── users/                 # Per-user configurations
│   └── hashwarlock/
│       ├── nixos.nix      # User account definition (empty, inherits from system.nix)
│       └── home.nix       # User's home-manager configuration
├── home/                  # Home-manager modules (user-level configs)
│   └── modules/           # Modular configs for individual programs
│       ├── common.nix     # Common home-manager imports
│       ├── hyprland.nix   # User-level Hyprland config
│       ├── gnome.nix      # User-level GNOME config
│       ├── git.nix, zsh.nix, tmux.nix, etc.
└── overlays/              # Nixpkgs overlays
    └── default.nix        # Unstable packages overlay (pkgs.unstable)
```

### Key Architectural Patterns

1. **Flake specialArgs**: The `username` and `outputs` are passed as `specialArgs` to enable parameterized configurations
2. **Home-manager integration**: Configured as a NixOS module, deployed automatically with `nixos-rebuild`
3. **Overlay system**: Unstable packages accessible via `pkgs.unstable.*` throughout configuration
4. **Catppuccin theming**: Macchiato flavor with lavender accent applied globally via catppuccin.nix modules
5. **Module imports**: System configs in `modules/`, user-level configs in `home/modules/`

### Current Host Configuration

- **Hostname**: `asus-g512lw` (defined in flake.nix)
- **Username**: `hashwarlock`
- **System version**: NixOS 24.05
- **Desktop environments**: Hyprland (primary), GNOME (secondary)
- **Display manager**: GDM

## Common Commands

### Building and Switching Configuration

```bash
# Rebuild and switch system configuration (requires sudo)
sudo nixos-rebuild switch --flake .#asus-g512lw

# Build without switching (test configuration)
sudo nixos-rebuild build --flake .#asus-g512lw

# Test configuration (activates but doesn't set as default boot)
sudo nixos-rebuild test --flake .#asus-g512lw

# Boot into new configuration without activating now
sudo nixos-rebuild boot --flake .#asus-g512lw
```

### Flake Operations

```bash
# Update flake inputs (update nixpkgs, home-manager, etc.)
nix flake update

# Update specific input only
nix flake lock --update-input nixpkgs

# Show flake metadata
nix flake show

# Check flake for errors
nix flake check
```

### Package Management

```bash
# Search for packages (stable)
nix search nixpkgs <package-name>

# Search in unstable
nix search nixpkgs-unstable <package-name>

# Garbage collection (free up disk space)
sudo nix-collect-garbage -d

# Optimize nix store
nix-store --optimize
```

### Home Manager (when not integrated as NixOS module)

Note: This configuration uses home-manager as a NixOS module, so changes apply via `nixos-rebuild`. Standalone home-manager commands are not needed.

## Development Workflow

1. **Adding new packages**: Edit `modules/system.nix` (system-level) or relevant `home/modules/*.nix` (user-level)
2. **Modifying desktop environment**: Edit `modules/hyprland.nix` or `modules/gnome.nix` and corresponding `home/modules/` configs
3. **Adding new host**: Create `hosts/<hostname>/default.nix` and add to `flake.nix` nixosConfigurations
4. **Using unstable packages**: Reference as `pkgs.unstable.<package>` (overlay defined in `overlays/default.nix`)
5. **Testing changes**: Use `nixos-rebuild test` to activate without committing to boot configuration

## Important Notes

- **State versions**: Current stateVersion is `24.05` - do not change unless migrating
- **Trusted users**: User `hashwarlock` is a trusted user (can use additional substituters)
- **Experimental features**: Flakes and nix-command are enabled globally
- **Garbage collection**: Automatic weekly GC deletes items older than 365 days
- **Unfree packages**: Allowed via `nixpkgs.config.allowUnfree = true`
