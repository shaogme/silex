ARG BASE_IMAGE=ghcr.io/shaogme/nixos-dockers/mise:latest
FROM ${BASE_IMAGE}

WORKDIR /root/workspace

# Copy custom entrypoint for automatic mise environment loading
COPY docker/entrypoint.sh /usr/local/bin/mise-entrypoint.sh
RUN chmod +x /usr/local/bin/mise-entrypoint.sh

# Enable non-interactive auto-confirm for mise
ENV MISE_YES=1

# Install system-level binaries via nix-env with BuildKit cache mount & binary mirror
RUN --mount=type=cache,target=/root/.cache/nix \
    --mount=type=cache,target=/tmp \
    echo "sandbox = false" >> /etc/nix/nix.conf && \
    nix-env --option sandbox false -iA \
        nixpkgs.bubblewrap \
        nixpkgs.fontconfig \
        nixpkgs.dejavu_fonts \
        nixpkgs.mesa \
        nixpkgs.firefox \
        nixpkgs.geckodriver \
        nixpkgs.zola

# Set the PATH and Headless / Fontconfig environment variables
ENV PATH=/root/.nix-profile/bin:$PATH \
    FONTCONFIG_PATH=/root/.nix-profile/etc/fonts \
    FONTCONFIG_FILE=/root/.nix-profile/etc/fonts/fonts.conf \
    MOZ_HEADLESS=1 \
    LIBGL_ALWAYS_SOFTWARE=1

# Configure fontconfig and silent Firefox headless preferences via enterprise policies
RUN set -e; \
    mkdir -p /etc/fonts; \
    if [ -f /root/.nix-profile/etc/fonts/fonts.conf ]; then \
        ln -sf /root/.nix-profile/etc/fonts/fonts.conf /etc/fonts/fonts.conf; \
    fi; \
    fc-cache -fv || true; \
    mkdir -p /etc/firefox/policies; \
    { \
        echo '{'; \
        echo '  "policies": {'; \
        echo '    "DisableTelemetry": true,'; \
        echo '    "DisableFirefoxStudies": true,'; \
        echo '    "DisablePocket": true,'; \
        echo '    "DisableFeedbackCommands": true,'; \
        echo '    "Preferences": {'; \
        echo '      "app.normandy.enabled": false,'; \
        echo '      "app.normandy.api_url": "",'; \
        echo '      "app.shield.optoutstudies.enabled": false,'; \
        echo '      "datareporting.healthreport.uploadEnabled": false,'; \
        echo '      "datareporting.policy.dataSubmissionEnabled": false,'; \
        echo '      "toolkit.telemetry.enabled": false,'; \
        echo '      "toolkit.telemetry.unified": false,'; \
        echo '      "services.settings.server": "",'; \
        echo '      "browser.safebrowsing.blockedURIs.enabled": false,'; \
        echo '      "browser.safebrowsing.downloads.enabled": false,'; \
        echo '      "browser.safebrowsing.malware.enabled": false,'; \
        echo '      "browser.safebrowsing.phishing.enabled": false,'; \
        echo '      "browser.search.suggest.enabled": false,'; \
        echo '      "signon.rememberSignons": false'; \
        echo '    }'; \
        echo '  }'; \
        echo '}'; \
    } > /etc/firefox/policies/policies.json

# 1. First copy only mise config files to maximize BuildKit layer cache hit rate
COPY .config/mise.toml /root/workspace/.config/mise.toml

# Detect mise configuration at build time and pre-install tools with BuildKit cache mounts
RUN --mount=type=cache,target=/root/.cache/mise \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    set -e; \
    echo "Pre-installing mise tools and Rust targets during docker build..."; \
    mise trust --all; \
    mise install rust; \
    mise install; \
    mise task run postinstall || true

# 2. Copy the rest of the workspace source code
COPY . /root/workspace/

# 3. Pre-fetch all Cargo dependencies from Cargo.lock and record hash
RUN set -e; \
    echo "Pre-fetching all Cargo dependencies from Cargo.lock..."; \
    mise exec -- cargo fetch --locked; \
    sha256sum Cargo.lock | awk '{print $1}' > /root/.cargo/.silex_cargo_lock_hash

# Set the custom entrypoint
ENTRYPOINT ["/usr/local/bin/mise-entrypoint.sh"]

# Default command passed to entrypoint (empty to trigger default behavior: sshd or bash)
CMD []

