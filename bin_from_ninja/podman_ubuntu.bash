#!/usr/bin/env bash
set -xeuo pipefail
podman build -t bin_from_ninja_ubuntu -f Containerfile_ubuntu .
podman image prune -f
podman run --rm bin_from_ninja_ubuntu backup -h
podman run --rm bin_from_ninja_ubuntu synchronize_backup -h
podman run --rm bin_from_ninja_ubuntu synchronize_partially -h
