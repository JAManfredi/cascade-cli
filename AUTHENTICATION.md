# Authentication Methods

Cascade supports multiple authentication methods to work with different corporate and personal setups.

## Configuration

### SSH Authentication (Recommended)
If your git remote uses SSH (e.g., `git@bitbucket.example.com:PROJECT/repo.git`):

```bash
# No additional configuration needed
# Cascade will use your SSH keys automatically
```

### Username + Token Authentication
For corporate environments or when using Personal Access Tokens:

```bash
# Set both username and token
ca config set bitbucket.username "your-username"
ca config set bitbucket.token "your-personal-access-token"
```

### Token-Only Authentication
Some Bitbucket setups use token as username:

```bash
# Set only the token
ca config set bitbucket.token "your-personal-access-token"
```

### Username + Password with Credential Helpers
For traditional username/password setups:

```bash
# Set username, password will be handled by git credential helpers
ca config set bitbucket.username "your-username"
```

## Authentication Priority

Cascade tries authentication methods in this order:

1. **SSH Key Authentication** (for SSH URLs)
2. **Username + Token** (if both are configured)
3. **Token as Username** (if only token is configured)
4. **Username Only** (uses system credential helpers for password)
5. **Default Credential Helper** (system git credential integration)
6. **Git CLI Fallback** (if git2 authentication fails)

## Troubleshooting

### Authentication Failures
If authentication fails, Cascade will automatically fall back to git CLI which often works better in corporate environments.

### Debug Authentication
Use verbose mode to see which authentication method is being used:

```bash
ca --verbose push
ca --verbose submit
```

### Creating Personal Access Tokens
For Bitbucket Server, create a token at:
```
https://your-bitbucket-server.com/plugins/servlet/access-tokens/manage
```

Required permissions: Repository Read, Repository Write

## Examples

### Corporate Setup with VPN
```bash
ca config set bitbucket.username "john.doe"
ca config set bitbucket.token "abc123..."
```

### Personal Setup with SSH
```bash
# No configuration needed if SSH is working
git remote -v  # Should show SSH URL
```

### Mixed Environment
```bash
# Works with both SSH and HTTPS remotes
ca config set bitbucket.username "your-username"
ca config set bitbucket.token "your-token"
```