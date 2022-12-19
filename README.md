
POCs written in Rust
====================

There is a finished POC and an unfinished one:

  - [`structured_concurrency`][] is adapted from [the code][] of the [Rust Book final project][],
    but has a few differences. Notably, it uses the structured concurrency paradigm.

  - [`bin_from_ninja`] is unfinished. It will be a project to compile Rust programs and deploy the
    binaries to `$HOME/bin` with a nice workflow, with a combination of make and [Ninja Build][].
    Current status:

    - [x] Finish the code of the [`backup`][] project: DONE.
    - [ ] Finish the code of the `synchronize_backup` project: TODO.
    - [ ] Improve the code which writes the `build.ninja` file: TODO.

    For now, I just published the `backup` project, but I will publish the rest soon.

[`structured_concurrency`]: ./structured_concurrency
[the code]: https://github.com/rust-lang/book/tree/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code
[Rust Book final project]: https://doc.rust-lang.org/stable/book/ch20-00-final-project-a-web-server.html
[`bin_from_ninja`]: ./bin_from_ninja
[Ninja Build]: https://ninja-build.org/
[`backup`]: ./bin_from_ninja/backup
