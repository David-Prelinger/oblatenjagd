FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Systemabhängigkeiten für grafische Rust-Spiele (Bevy / winit / wgpu / Audio)
# plus Mesa-Software-Rendering, damit das Spiel ohne GPU im Container läuft.
# chromium: headless browser for `cargo xtask wasm-smoke` (SwiftShader WebGL).
# xdotool: send key presses to the game window on DISPLAY=:1 (used by the VNC smoke test, F12 checks).
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        jq \
        clang \
        mold \
        libasound2-dev \
        libudev-dev \
        libx11-dev \
        libxcursor-dev \
        libxrandr-dev \
        libxi-dev \
        libxkbcommon-dev \
        libxkbcommon-x11-0 \
        libwayland-dev \
        libvulkan1 \
        mesa-vulkan-drivers \
        libgl1-mesa-dri \
        libegl1 \
        vulkan-tools \
        mesa-utils \
        chromium \
        xdotool \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
