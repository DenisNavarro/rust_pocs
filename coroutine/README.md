
`coroutine`
===========

This POC shows a coroutine example and compares it to generic programming for decoupling algorithm
from I/O.

Remark: In the context of network protocols, Python and Rust developers call [sans I/O][] this
decoupling.

In this POC, a simple CLI is implemented in several ways. The CLI takes an UTF-8 file path argument
and does the following:

```rust
/// If the file has 42 bytes or more, move it by appending a suffix.
///
/// The suffix is `.YYYY-MM-DD.number` with `YYYY-MM-DD` the current date and
/// `number` the smallest positive integer such that the destination path does
/// not exist before the move.
```

[sans I/O]: https://sans-io.readthedocs.io/

## [`basic_renamer`](./basic_renamer)

This package implements the CLI without decoupling algorithm from I/O.

## [`generic_renamer`](./generic_renamer)

In this package, the algorithm is implemented in [a `no_std` library](./generic_renamer/lib.rs)
which is used in [a CLI implemented in sync Rust](./generic_renamer/main.rs).

The algorithm code uses generic programming. The main drawback of generic programming in Rust is
that the code cannot be async agnostic. To do this, the [keyword generics][] are needed.

[keyword generics]: https://blog.rust-lang.org/inside-rust/2023/02/23/keyword-generics-progress-report-feb-2023.html

## [`renamer`](./renamer)

In this package, the algorithm is implemented in [a `no_std` library](./renamer/lib.rs) which is
used in [a CLI implemented in sync Rust](./renamer/sync_renamer.rs) and
[a CLI implemented in async Rust](./renamer/async_renamer.rs).

The algorithm code defines an async-agnostic coroutine. The I/O effect is handled by the caller.

Furthermore, this coroutine does not yield any instance of `std::result::Result`. The error
handling is done in the caller.

The main drawback of writing a coroutine in stable Rust is that the code is less readable because
it looks like a state machine such that each state change breaks [structured programming][].

To write readable coroutines, some language support is needed.

Remark: The [`effing-mad`][] crate allows to define a coroutine in a more readable way. It has
[an async-agnostic code example][], but it requires nightly Rust:

```rust
effing_mad::effects! {
    HttpRequest {
        fn get(url: &'static str) -> String;
    }
}

// this function does not specify whether the request happens synchronously or asynchronously
#[effectful(HttpRequest)]
fn example() -> usize {
    let body = yield HttpRequest::get("http://example.com");
    body.len()
}
```

[structured programming]: https://en.wikipedia.org/wiki/Structured_programming
[`effing-mad`]: https://crates.io/crates/effing-mad
[an async-agnostic code example]: https://github.com/rosefromthedead/effing-mad/blob/v0.1.0/examples/sync-and-async.rs
