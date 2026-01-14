---
name: nixos-operations
description: Knowledge about NixOS system operations - rebuild, rollback, garbage collection
triggers:
  - rebuild
  - nixos-rebuild
  - rollback
  - generation
  - generations
  - garbage collection
  - nix-collect-garbage
  - switch
  - boot
  - test
---

# NixOS Operations Knowledge

## Rebuilding the System

### Basic rebuild commands

```bash
# Apply changes immediately
sudo nixos-rebuild switch --flake .#hostname

# Build only, don't apply
sudo nixos-rebuild build --flake .#hostname

# Apply on next boot only
sudo nixos-rebuild boot --flake .#hostname

# Test without persisting (reverts on reboot)
sudo nixos-rebuild test --flake .#hostname
```

### Rebuild options

```bash
# Show build log
sudo nixos-rebuild switch --flake .#hostname --show-trace

# Upgrade inputs first
sudo nix flake update && sudo nixos-rebuild switch --flake .#hostname

# Use remote builder
sudo nixos-rebuild switch --flake .#hostname --builders "ssh://builder"
```

## Generations & Rollback

### List generations
```bash
# System generations
sudo nix-env --list-generations -p /nix/var/nix/profiles/system

# Or using nixos-rebuild
nixos-rebuild list-generations
```

### Rollback

```bash
# Roll back to previous generation
sudo nixos-rebuild switch --rollback

# Switch to specific generation
sudo nix-env --switch-generation 42 -p /nix/var/nix/profiles/system
sudo /nix/var/nix/profiles/system/bin/switch-to-configuration switch
```

### Delete old generations
```bash
# Delete generations older than 30 days
sudo nix-env --delete-generations +30d -p /nix/var/nix/profiles/system

# Keep only last 5 generations
sudo nix-env --delete-generations +5 -p /nix/var/nix/profiles/system
```

## Garbage Collection

### Manual cleanup
```bash
# Collect garbage (unused packages)
sudo nix-collect-garbage

# Also delete old generations first
sudo nix-collect-garbage -d

# Delete old generations AND run GC
sudo nix-collect-garbage --delete-older-than 30d
```

### Optimize store
```bash
# Deduplicate identical files in store
sudo nix-store --optimize
```

### Automatic GC
In `configuration.nix`:

```nix
nix.gc = {
  automatic = true;
  dates = "weekly";
  options = "--delete-older-than 30d";
};

# Also auto-optimize
nix.optimise.automatic = true;
```

## Troubleshooting

### Check current generation
```bash
readlink /nix/var/nix/profiles/system
```

### See what will be built
```bash
nix build .#nixosConfigurations.hostname.config.system.build.toplevel --dry-run
```

### Debug eval errors
```bash
nix eval .#nixosConfigurations.hostname.config.system.build.toplevel --show-trace
```

### Check disk usage
```bash
# Total nix store size
du -sh /nix/store

# Largest store paths
nix path-info -rSh /run/current-system | sort -hk2 | tail -20
```

## Best Practices

1. **Always test first** - Use `nixos-rebuild test` before `switch`
2. **Keep recent generations** - Don't delete all, keep fallbacks
3. **Regular GC** - Set up automatic garbage collection
4. **Commit before rebuild** - Track what changed if something breaks
5. **Check logs** - `journalctl -b` after rebuild for issues
