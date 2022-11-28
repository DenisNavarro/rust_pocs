
`structured_concurrency`
========================

This POC is adapted from the [Rust Book final project][], but has a few differences. Notably, it
uses the structured concurrency paradigm.

Here are the differences:

## Structured concurrency

The best introduction I know to structured concurrency is an excellent article from
Nathaniel J. Smith: [Notes on structured concurrency, or: Go statement considered harmful][].

The code from the [Rust Book final project][] does not use structured concurrency. Indeed, it
calls `std::thread::spawn`. But then the borrow checker requires to create an
`Arc<Mutex<Receiver<Job>>>` instead of a `Mutex<Receiver<Job>>`.

Fortunately, [Rust 1.63.0][] introduced `std::thread::scope`, which allows to use the structured
concurrency paradigm!

## Remove a useless `Option`

In the original code, the `Worker::thread` attribute was an `Option<thread::JoinHandle<()>>`:
<https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs#L66>

Why `Option`? Because `ThreadPool::drop` called `Option::take`:
<https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs#L57>

But it was useless. A better solution is to call `Vec::drain`.

## Use `NonZeroUsize`

In the original code, `ThreadPool::new` panicked if the size (thread count) was 0:
<https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs#L20>

[Rust 1.28.0][] introduced `std::num::NonZeroUsize`. The typing is stronger.

## Make Clippy happy

The other changes in the code are just to make Clippy happy.  
I like using `#![warn(clippy::nursery, clippy::pedantic)]`.

## Add a Makefile

With a Makefile, the code is easier to update.

I did not add automated tests, so the testing is still done manually by opening:

  + <http://127.0.0.1:7878/>
  + <http://127.0.0.1:7878/sleep>
  + <http://127.0.0.1:7878/invalid-path>

[Rust Book final project]: https://github.com/rust-lang/book/tree/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code
[Notes on structured concurrency, or: Go statement considered harmful]: https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/
[Rust 1.28.0]: https://blog.rust-lang.org/2018/08/02/Rust-1.28.html
[Rust 1.63.0]: https://blog.rust-lang.org/2022/08/11/Rust-1.63.0.html
