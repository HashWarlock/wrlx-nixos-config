---
name: nixos-flakes
description: Knowledge about NixOS flakes structure and configuration
triggers:
  - flake
  - flakes
  - nix flake
  - flake.nix
  - flake inputs
  - flake outputs
---

# NixOS Flakes Knowledge

## Flake Structure

A NixOS flake is defined in `flake.nix` with three main sections:

### Inputs
External dependencies pinned to specific versions via `flake.lock`:

```nix
inputs = {
  nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
  nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
  home-manager = {
    url = "github:nix-community/home-manager/release-24.05";
    inputs.nixpkgs.follows = "nixpkgs";
  };
};
```

**Key patterns:**
- `follows` - makes an input use another input's nixpkgs (reduces duplication)
- Version pinning happens automatically via `flake.lock`

### Outputs
A function that receives inputs and returns configurations:

```nix
outputs = { self, nixpkgs, home-manager, ... }@inputs: {
  nixosConfigurations.hostname = nixpkgs.lib.nixosSystem {
    system = "x86_64-linux";
    modules = [ ./configuration.nix ];
    specialArgs = { inherit inputs; };
  };

  devShells.x86_64-linux.default = pkgs.mkShell {
    packages = [ pkgs.rustc pkgs.cargo ];
  };
};
```

**Output types:**
- `nixosConfigurations` - Full NixOS system configs
- `devShells` - Development environments (`nix develop`)
- `packages` - Packages to build (`nix build`)
- `overlays` - Package modifications
- `homeConfigurations` - Standalone home-manager configs

### specialArgs
Pass extra arguments to modules:

```nix
specialArgs = {
  inherit inputs;
  username = "myuser";
  hostname = "myhost";
};
```

These are available in all modules via the function argument.

## Common Operations

### Update all inputs
```bash
nix flake update
```

### Update specific input
```bash
nix flake lock --update-input nixpkgs
```

### Check flake validity
```bash
nix flake check
```

### Show flake outputs
```bash
nix flake show
```

## DevShells

Create reproducible development environments:

```nix
devShells.default = pkgs.mkShell {
  packages = with pkgs; [
    rustc cargo rust-analyzer
    nodejs npm
  ];

  shellHook = ''
    echo "Development environment ready!"
  '';

  # Environment variables
  RUST_BACKTRACE = "1";
};
```

Enter with: `nix develop` or `nix develop .#shellName`

## Best Practices

1. **Pin all inputs** - Use `flake.lock` for reproducibility
2. **Use `follows`** - Reduce duplication of nixpkgs
3. **Modularize** - Split large configs into separate files
4. **Use overlays** - For package customizations
5. **Document specialArgs** - Make module dependencies clear
