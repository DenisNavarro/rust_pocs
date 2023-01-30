
POCs written in Rust
====================

Currently, there are 2 POCs:

  - [`structured_concurrency`][] is adapted from [the code][] of the [Rust Book final project][],
    but has a few differences. Notably, it uses the structured concurrency paradigm.

  - [`bin_from_ninja`] combines Make and [Ninja][] to compile and check the Rust programs
    [`backup`][] and [`synchronize_backup`][] and deploy the binaries to `$HOME/bin`, thanks to
    the `build.ninja` file written by [`ninja_bootstrap`][].  
    Requirements: Unix, Make, Ninja, `cp` (for `backup`) and `rsync` (for `synchronize_backup`).

[`structured_concurrency`]: ./structured_concurrency
[the code]: https://github.com/rust-lang/book/tree/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code
[Rust Book final project]: https://doc.rust-lang.org/stable/book/ch20-00-final-project-a-web-server.html
[`bin_from_ninja`]: ./bin_from_ninja
[Ninja]: https://ninja-build.org/
[`backup`]: ./bin_from_ninja/backup/src/main.rs
[`synchronize_backup`]: ./bin_from_ninja/synchronize_backup/src/main.rs
[`ninja_bootstrap`]: ./bin_from_ninja/ninja_bootstrap/src/main.rs
