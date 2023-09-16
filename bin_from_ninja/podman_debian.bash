#!/usr/bin/env bash
set -xeuo pipefail
podman build -t bin_from_ninja_debian -f Containerfile_debian .
podman image prune -f
podman run --rm bin_from_ninja_debian backup -h
podman run --rm bin_from_ninja_debian synchronize_backup -h
podman run --rm bin_from_ninja_debian synchronize_partially -h
