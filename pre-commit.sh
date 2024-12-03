#!/bin/sh

set -e

if ! git diff --cached --quiet -- bin_from_ninja; then (
    cd bin_from_ninja
    ./podman.bash
) fi

if ! git diff --cached --quiet -- coroutine; then (
    cd coroutine
    cargo +1.82.0 fmt --all --check
    cargo +1.82.0 clippy --all-features --all-targets --locked --workspace -- -D warnings
    cargo +1.82.0 test --locked --workspace
    cargo +1.83.0 fmt --all --check
    cargo +1.83.0 clippy --all-features --all-targets --locked --workspace -- -D warnings
    cargo +1.83.0 test --locked --workspace
) fi

if ! git diff --cached --quiet -- structured_concurrency; then (
    cd structured_concurrency
    cargo +1.81.0 fmt --check
    cargo +1.81.0 clippy --all-features --all-targets --locked -- -D warnings
    cargo +1.81.0 test --locked
) fi
