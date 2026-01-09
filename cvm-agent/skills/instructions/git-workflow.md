---
name: git-workflow
description: Guidelines for git operations in this repository
triggers:
  - commit changes
  - push code
  - git status
  - version control
---

# Git Workflow Guidelines

## Before Making Changes

1. Check current status: `git status`
2. Ensure you're on the correct branch
3. Pull latest changes if working with remote

## Committing Changes

### Commit Message Format

```
<type>: <short description>

<optional longer description>

Co-Authored-By: Claude <noreply@anthropic.com>
```

Types:
- `feat:` - New feature
- `fix:` - Bug fix
- `chore:` - Maintenance tasks
- `docs:` - Documentation only
- `refactor:` - Code restructuring

### Pre-commit Checklist

1. Review all changes with `git diff`
2. Ensure no secrets or credentials are included
3. Verify NixOS syntax is valid
4. Stage only relevant files

## Safe Operations

- Always commit before destructive operations
- Use branches for experimental changes
- Push regularly to preserve work
- Never force-push to main/master
