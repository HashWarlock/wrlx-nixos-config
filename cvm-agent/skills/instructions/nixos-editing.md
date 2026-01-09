---
name: nixos-editing
description: Guidelines for editing NixOS configuration files
triggers:
  - edit config
  - modify nix
  - change system
  - add package
  - remove package
---

# NixOS Configuration Editing Guidelines

When editing NixOS configuration files in this repository:

## File Organization

1. **System-level configs** go in `modules/`
   - `system.nix` - Core system packages and settings
   - `hyprland.nix`, `gnome.nix`, `i3.nix` - Desktop environments
   - `cvm-agent.nix` - CVM Agent service configuration

2. **User-level configs** go in `home/modules/`
   - Program-specific configs (git.nix, zsh.nix, tmux.nix)
   - Desktop environment user settings

3. **Host-specific configs** go in `hosts/<hostname>/`
   - `default.nix` - Host entry point
   - `hardware-configuration.nix` - Hardware-specific settings

## Best Practices

1. **Read before editing** - Always read the current file to understand structure
2. **Prefer modification** - Modify existing modules over creating new ones
3. **Use unstable overlay** - Reference bleeding-edge packages as `pkgs.unstable.*`
4. **Follow patterns** - Match existing code style and organization
5. **Explain changes** - After editing, explain what changed and why

## Package Management

- Add system packages in `modules/system.nix` under `environment.systemPackages`
- Add user packages in relevant `home/modules/*.nix` files
- For bleeding-edge versions, use `pkgs.unstable.<package>`

## Testing Changes

Before rebuilding:
1. Verify syntax: `nix flake check`
2. Build without activating: `nixos-rebuild build --flake .#<host>`
3. Test activation: `nixos-rebuild test --flake .#<host>`
4. Finally switch: `nixos-rebuild switch --flake .#<host>`
