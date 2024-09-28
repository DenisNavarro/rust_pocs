
POCs written in Rust
====================

Here are the POCs in the chronological order:

  - [`structured_concurrency`][] is adapted from [the code][] of the [Rust Book final project][],
    but has a few differences. Notably, it uses the structured concurrency paradigm.

  - [`bin_from_ninja`] combines Make and [Ninja][] to compile and check the Rust programs
    [`backup`][], [`synchronize_backup`][] and [`synchronize_partially`][], and deploy the
    binaries to `$HOME/bin`, thanks to the `build.ninja` file written by [`ninja_bootstrap`][].  
    It also uses [Pixi][], but this dependency is optional.

  - [`coroutine`] shows a coroutine example and compares it to generic programming for decoupling
    algorithm from I/O.

[`structured_concurrency`]: ./structured_concurrency
[the code]: https://github.com/rust-lang/book/tree/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code
[Rust Book final project]: https://doc.rust-lang.org/stable/book/ch20-00-final-project-a-web-server.html
[`bin_from_ninja`]: ./bin_from_ninja
[Ninja]: https://ninja-build.org/
[`backup`]: ./bin_from_ninja/backup/main.rs
[`synchronize_backup`]: ./bin_from_ninja/synchronize_backup/main.rs
[`synchronize_partially`]: ./bin_from_ninja/synchronize_partially/main.rs
[`ninja_bootstrap`]: ./bin_from_ninja/ninja_bootstrap/main.rs
[Pixi]: https://pixi.sh/
[`coroutine`]: ./coroutine
