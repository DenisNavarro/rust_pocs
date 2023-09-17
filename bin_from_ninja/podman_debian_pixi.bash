#!/usr/bin/env bash
set -xeuo pipefail
podman build -t bin_from_ninja_debian_pixi -f Containerfile_debian_pixi .
podman image prune -f
podman run --rm bin_from_ninja_debian_pixi backup -h
podman run --rm bin_from_ninja_debian_pixi synchronize_backup -h
podman run --rm bin_from_ninja_debian_pixi synchronize_partially -h
