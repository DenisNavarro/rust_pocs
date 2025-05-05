#!/usr/bin/env bash
set -xeuo pipefail
podman build -t bin_from_ninja_msrv -f Dockerfile_msrv .
podman build -t bin_from_ninja .
podman image prune -f
podman run --rm bin_from_ninja_msrv backup -h
podman run --rm bin_from_ninja_msrv synchronize_backup -h
podman run --rm bin_from_ninja_msrv synchronize_partially -h
podman run --rm bin_from_ninja backup -h
podman run --rm bin_from_ninja synchronize_backup -h
podman run --rm bin_from_ninja synchronize_partially -h
