#!/bin/sh
set -e

cd "$(git rev-parse --show-toplevel)"

if ! git diff --cached --quiet -- bin_from_ninja; then
    ./bin_from_ninja/podman.bash
fi

if ! git diff --cached --quiet -- coroutine; then (
    cd coroutine
    cargo +1.87.0 test --locked --workspace
    cargo +1.96.0 fmt --all --check
    cargo +1.96.0 clippy --all-features --all-targets --locked --workspace -- -D warnings
    cargo +1.96.0 test --locked --workspace
) fi

if ! git diff --cached --quiet -- structured_concurrency; then (
    cd structured_concurrency
    cargo +1.85.1 check --locked
    cargo +1.96.0 fmt --check
    cargo +1.96.0 clippy --all-features --all-targets --locked -- -D warnings
    cargo +1.96.0 check --locked
) fi
