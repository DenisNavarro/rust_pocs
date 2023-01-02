
`bin_from_ninja`
================

This POC combines Make and [Ninja][] to compile and check the Rust programs [`backup`][] and
[`synchronize_backup`][] and deploy the binaries to `$HOME/bin`.

Requirements: Unix, Make, Ninja, `cp` (for `backup`) and `rsync` (for `synchronize_backup`).

## Worflow

The idea is that, instead of launching `cargo clippy`, `cargo test`, etc., the developer launches
one of these commands:

  - `make fmt`: For each project, if not done yet, reformat the code (with `cargo fmt`).
  - `make check` or just `make`: For each project, if not done yet, reformat the code and check it
    (with `cargo clippy` and `cargo test`).
  - `make all`: For each project, if not done yet, reformat the code, check it, compile it in
    release mode and, if all is good, deploy the up-to-date binary to `$HOME/bin`.

In most cases, the developer just launches `make`.

When the code is ready to be deployed, `make all` can be launched.

`make fmt` may be useless if the developer can already reformat the current Rust file with a
keystroke.

Under the hood, these Make commands call Ninja to launch the underlying commands in parallel.

## Ninja

Why did I choose to use Ninja? Make can also launch commands in parallel with `make -j`, but the
output is interleaved, so unreadable. Ninja has a much nicer output.

What is Ninja? Ninja is a fast minimalist build system. Compared to Make, it has very few
features. A Ninja build file (like a `Makefile` but for Ninja instead of Make) is typically
written by a program instead of by a human.

In the current POC, the [`ninja_bootstrap`][] program writes the `build.ninja` file.

Remark for projects with more complex workflows: If a complex workflow can be automated with a
`Makefile` which uses advanced features of Make and if, like Matt Rickard, you think that
[every sufficiently advanced configuration language is wrong][], then you may prefer to use a
regular programming language which writes a Ninja build file.

## [`backup`][]

This is a simple CLI.

```rust
/// Copy directories and files by adding a suffix which depends on the current datetime.
/// Tested on Linux.
///
/// For example, on 2022-12-13 14:15:16, `backup /path/to/directory /path/to/file` copies
/// `/path/to/directory` to `/path/to/directory_2022-12-13-14h15` and
/// `/path/to/file` to `/path/to/file_2022-12-13-14h15`.
///
/// `backup` follows command-line symlinks.
```

## [`synchronize_backup`][]

This is a simple CLI I execute every evening.

```rust
/// Synchronize a directory with a backup directory by renaming a suffix and calling rsync.
/// Tested on Linux.
///
/// For example, on 2022-12-13 14:15:16, if the directory `/my/hard/drive/foo_2022-08-09-10h11`
/// exists, then `synchronize_backup /path/to/foo /my/hard/drive` renames
/// `/my/hard/drive/foo_2022-08-09-10h11` to `/my/hard/drive/foo_2022-12-13-14h15` and then calls
/// `time rsync -aAXHv --delete --stats -- /path/to/foo/ /my/hard/drive/foo_2022-12-13-14h15`.
///
/// If there is no directory candidate to rename, `rsync` is called anyway and creates a new one.
/// If there are several candidates, no one is renamed, `rsync` is not called and an error code is
/// returned.
///
/// `synchronize_backup` follows command-line symlinks.
///
/// In the current implementation, the source directory path must be a valid UTF-8 sequence.
```

## [`ninja_bootstrap`][]

This program writes the `build.ninja` file.

To make the `main.rs` code concise, I wrote builders in `lib.rs`, but the builder code is very big.

[Ninja]: https://ninja-build.org/
[`backup`]: ./backup/src/main.rs
[`synchronize_backup`]: ./synchronize_backup/src/main.rs
[`ninja_bootstrap`]: ./ninja_bootstrap
[every sufficiently advanced configuration language is wrong]: https://matt-rickard.com/advanced-configuration-languages-are-wrong
