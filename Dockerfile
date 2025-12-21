# Build arguments
ARG VARIANT=debian-13
ARG NPM_VERSION=10.9.2
# Rust version (stable, nightly, or specific version like 1.92.0)
ARG RUST_VERSION=1.92.0

# Base: Microsoft Dev Containers base image for Debian 13 (Trixie)
# https://mcr.microsoft.com/en-us/product/devcontainers/base/about
FROM mcr.microsoft.com/devcontainers/base:${VARIANT} AS base

# Re-declare build arguments for this stage (ARGs don't persist across FROM)
ARG NPM_VERSION
ARG RUST_VERSION

# Install build dependencies often needed by Rust crates
# - lldb: debugging via CodeLLDB
# - build-essential, pkg-config, cmake: native builds
# - libssl-dev, libclang-dev: common native deps (OpenSSL/clang bindgen)
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
       build-essential \
       pkg-config \
       cmake \
       curl \
       git \
       ca-certificates \
       lldb \
       libssl-dev \
       libclang-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Install Node.js and npm
USER root
RUN export DEBIAN_FRONTEND=noninteractive \
    && curl -fsSL https://deb.nodesource.com/setup_lts.x -o nodesource_setup.sh \
    && bash nodesource_setup.sh \
    && rm nodesource_setup.sh \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && if [ "${NPM_VERSION}" != "bundled" ]; then npm install -g "npm@${NPM_VERSION}"; fi \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install Rust for all users
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN set -eux \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- \
       -y \
       --profile minimal \
       --no-modify-path \
    && rustup component add rustfmt clippy \
    && rustup toolchain install ${RUST_VERSION} --profile minimal \
    && rustup default ${RUST_VERSION} \
    && chmod -R a+w $RUSTUP_HOME $CARGO_HOME \
    && rustup --version \
    && cargo --version \
    && rustc --version

# CI stage for continuous integration builds
FROM base AS ci

# Add any CI-specific tools here
# This stage includes Rust, Node.js, and common build tools
# but excludes heavy development dependencies like WSJTX

# Development stage with WSJTX dependencies (for devcontainer)
FROM base AS devcontainer

# Install WSJTX build dependencies
# - gfortran: Fortran compiler for WSJTX source
# - libboost-all-dev: Boost libraries for WSJTX
# - libfftw3-dev: FFT library for signal processing
# - libudev-dev, libusb-1.0-0-dev: USB device support
# - qt5 packages: Qt GUI framework for WSJTX
USER root
RUN apt-get update && apt-get install -y --no-install-recommends \
    gfortran \
    libboost-all-dev \
    libfftw3-dev \
    libudev-dev \
    libusb-1.0-0-dev \
    qtbase5-dev \
    qtmultimedia5-dev \
    qttools5-dev \
    libqt5multimedia5-plugins \
    libqt5serialport5-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Install Claude Code for non-root user
USER vscode
RUN curl -fsSL https://claude.ai/install.sh | bash
