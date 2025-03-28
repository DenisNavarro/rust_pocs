#!/bin/sh

set -e

if ! git diff --cached --quiet -- bin_from_ninja; then (
    cd bin_from_ninja
    ./podman.bash
) fi

if ! git diff --cached --quiet -- coroutine; then (
    cd coroutine
    cargo +1.85.0 test --locked --workspace
    cargo +1.85.1 fmt --all --check
    cargo +1.85.1 clippy --all-features --all-targets --locked --workspace -- -D warnings
    cargo +1.85.1 test --locked --workspace
) fi

if ! git diff --cached --quiet -- structured_concurrency; then (
    cd structured_concurrency
    cargo +1.85.0 test --locked
    cargo +1.85.1 fmt --check
    cargo +1.85.1 clippy --all-features --all-targets --locked -- -D warnings
    cargo +1.85.1 test --locked
) fi
