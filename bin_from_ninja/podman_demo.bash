#!/usr/bin/env bash
set -xeuo pipefail
podman build -t bin_from_ninja_image .
podman image prune -f
podman run --rm bin_from_ninja_image backup -h
podman run --rm bin_from_ninja_image synchronize_backup -h
podman run --rm bin_from_ninja_image synchronize_partially -h
