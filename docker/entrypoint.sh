#!/usr/bin/env bash
set -e

# ==========================================
# Mise Development Container Entrypoint
# ==========================================

WORKSPACE="${WORKSPACE_DIR:-/root/workspace}"

if [ -d "$WORKSPACE" ]; then
    cd "$WORKSPACE"
fi

# Candidate configuration files in order of precedence:
# 1. mise.local.toml
# 2. mise.toml
# 3. mise/config.toml
# 4. .mise/config.toml
# 5. .mise/conf.d/*.toml
# 6. .config/mise.toml
# 7. .config/mise/config.toml
# 8. .config/mise/conf.d/*.toml

CONFIG_FOUND=0
FOUND_PATH=""

CANDIDATE_FILES=(
    "mise.local.toml"
    "mise.toml"
    "mise/config.toml"
    ".mise/config.toml"
    ".config/mise.toml"
    ".config/mise/config.toml"
)

for cfg in "${CANDIDATE_FILES[@]}"; do
    if [ -f "$cfg" ]; then
        CONFIG_FOUND=1
        FOUND_PATH="$cfg"
        break
    fi
done

if [ $CONFIG_FOUND -eq 0 ]; then
    for dir in ".mise/conf.d" ".config/mise/conf.d"; do
        if [ -d "$dir" ] && compgen -G "$dir/*.toml" > /dev/null; then
            CONFIG_FOUND=1
            FOUND_PATH="$dir/*.toml"
            break
        fi
    done
fi

export PNPM_HOME="${PNPM_HOME:-/root/.local/share/pnpm}"
export PATH="$PNPM_HOME/bin:$PNPM_HOME:/root/.cargo/bin:/root/.nix-profile/bin:$PATH"
export MISE_YES=1
export FONTCONFIG_PATH="${FONTCONFIG_PATH:-/root/.nix-profile/etc/fonts}"
export FONTCONFIG_FILE="${FONTCONFIG_FILE:-/root/.nix-profile/etc/fonts/fonts.conf}"
export MOZ_HEADLESS=1
export LIBGL_ALWAYS_SOFTWARE=1

# Ensure ~/.bashrc hooks mise for interactive subshells and VS Code terminal sessions
if [ ! -f /root/.bashrc ] || ! grep -q "mise activate" /root/.bashrc; then
    echo 'eval "$(mise activate bash)"' >> /root/.bashrc
fi

if [ $CONFIG_FOUND -eq 1 ]; then
    echo "[mise-entrypoint] Found mise config ($FOUND_PATH). Initializing environment..."
    mise trust --all 2>/dev/null || true
    echo "[mise-entrypoint] Installing tools via mise..."
    mise install || true

    # Check and synchronize Cargo dependencies if Cargo.lock has changed
    if [ -f "Cargo.lock" ]; then
        LOCK_HASH_FILE="/root/.cargo/.silex_cargo_lock_hash"
        CURRENT_HASH=$(sha256sum Cargo.lock 2>/dev/null | awk '{print $1}')
        SAVED_HASH=$(cat "$LOCK_HASH_FILE" 2>/dev/null || echo "")

        if [ -n "$CURRENT_HASH" ] && [ "$CURRENT_HASH" != "$SAVED_HASH" ]; then
            echo "[mise-entrypoint] Cargo.lock change detected."
            echo "[mise-entrypoint] Pre-fetching incremental dependencies..."
            mise exec -- cargo fetch --locked 2>/dev/null || true
            mkdir -p /root/.cargo
            echo "$CURRENT_HASH" > "$LOCK_HASH_FILE"
            echo "[mise-entrypoint] Cargo dependencies synchronized."
        fi
    fi

    # Load mise environment variables into current shell so child processes inherit them
    eval "$(mise env -s bash 2>/dev/null || true)"
    echo "[mise-entrypoint] Mise environment ready."
else
    echo "[mise-entrypoint] No mise configuration found in workspace ($WORKSPACE)."
    echo "[mise-entrypoint] Skipping pre-installation."
fi

# Hand over execution to the base NixOS container entrypoint
exec /bin/entrypoint.sh "$@"
