
`coroutine`
===========

This POC shows various ways to decouple algorithm from I/O in Rust. Some of them use a coroutine.

There are several ways to abstract I/O operations (for example `std::fs::exists`) in an algorithm:
- The simplest way, when it is possible, is to do the I/O operations outside the algorithm.
  Some inputs of the algorithm can be gathered from I/O operations executed before the algorithm.
  Some outputs of the algorithm can describe I/O operations which will be executed after.
- The most known way is to define the algorithm as a function and to give it the I/O operations as
  callbacks or anything similar to a set of callbacks (for example an interface in OO languages).
  When a callback has effects, for example being async or being able to raise an error, the
  algorithm as a function has to manage these effects (most often by propagating them). Generic
  programming is handy to make the code reusable.
- Another way is to define the algorithm as a coroutine and let the caller handle I/O operations.
  For example, when the algorithm needs to know the result of `std::fs::exists`, it gives a path
  to the caller. Then, the caller calls `std::fs::exists` on this path and sends back the boolean
  to the coroutine. A coroutine can be seen as a generalization of an iterator, except that the
  `next` method is named `resume` and may require a parameter whose type depends on the coroutine
  state. In most cases, the coroutine does not manage effects itself. For example, if
  `std::fs::exists` fails, instead of sending an error to the coroutine, then the caller typically
  destroys the coroutine and manages the error itself.
- Another way is to define the algorithm as an abstract syntax tree. The caller defines an
  interpreter to execute it. But this way is not covered here.

In this POC, a simple CLI is implemented in 5 ways. The CLI takes a UTF-8 file path argument
and does the following:

```rust
/// If the file has 42 bytes or more, move it by appending a suffix.
///
/// The suffix is `.YYYY-MM-DD.number` with `YYYY-MM-DD` the current date and
/// `number` the smallest positive integer such that the destination path does
/// not exist before the move.
```

Here are the 5 implementations, from the simplest to the most complex:

## [`basic_renamer`](./basic_renamer)

This package implements the CLI without decoupling algorithm from I/O, except getting the current
datetime.

In the unit tests, the algorithm is tested in a temporary directory.

## [`simple_generic_renamer`](./simple_generic_renamer)

The algorithm is a function defined in a `no_std` library with generic programming in sync Rust.

The size of the file is a function parameter. The output of the function describes the optional
rename operation. The other I/O operations are done by callbacks. The current datetime could have
been a function parameter, but is retrieved by callback to avoid a system call if the current
datetime is not needed.

The error type is a generic parameter. In the unit tests, the error type is `Infallible`.

## [`generic_renamer`](./generic_renamer)

The algorithm is a function defined in [a `no_std` library](./generic_renamer/lib.rs) with generic
programming in async Rust.

It is used in [a CLI implemented in sync Rust](./generic_renamer/generic_sync_renamer.rs) and
[a CLI implemented in async Rust](./generic_renamer/generic_async_renamer.rs).

Rust does not have [keyword generics][] so the callbacks must return futures and the sync caller
code uses `block_on`.

[keyword generics]: https://blog.rust-lang.org/inside-rust/2023/02/23/keyword-generics-progress-report-feb-2023.html

## [`renamer`](./renamer)

The algorithm is a coroutine defined in [a `no_std` `async` agnostic library](./renamer/lib.rs).

It is used in [a CLI implemented in sync Rust](./renamer/sync_renamer.rs) and
[a CLI implemented in async Rust](./renamer/async_renamer.rs).

This coroutine is a handwritten machine state.

The main drawback of a handwritten machine state is that it hinders code readability.
Indeed, each state change breaks [structured programming][].

[structured programming]: https://en.wikipedia.org/wiki/Structured_programming

## [`corophage_renamer`](./corophage_renamer)

The algorithm is a coroutine defined in
[a `no_std` `async` agnostic library](./corophage_renamer/lib.rs).

It is used in [a CLI implemented in sync Rust](./corophage_renamer/corophage_sync_renamer.rs) and
[a CLI implemented in async Rust](./corophage_renamer/corophage_async_renamer.rs).

This coroutine uses the [`corophage`][] crate from Romain Ruetschi.

Thanks to `corophage`, the algorithm is far more readable than my handwritten machine state from my
[`renamer`](./renamer) package. The downside is that error handling is more painful in the caller
code.

Remark: There is also the [`effing-mad`][] crate which allows writing an async-agnostic coroutine
([async-agnostic code example][]). But it requires nightly Rust and is not maintained.

I wish Rust had language support to write coroutines more easily.

[`corophage`]: https://crates.io/crates/corophage
[`effing-mad`]: https://crates.io/crates/effing-mad
[async-agnostic code example]: https://github.com/rosefromthedead/effing-mad/blob/v0.1.0/examples/sync-and-async.rs
