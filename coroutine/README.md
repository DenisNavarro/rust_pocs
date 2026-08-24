
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

[`corophage`]: https://crates.io/crates/corophage
[`effing-mad`]: https://crates.io/crates/effing-mad
[async-agnostic code example]: https://github.com/rosefromthedead/effing-mad/blob/v0.1.0/examples/sync-and-async.rs

## Decoupling algorithm from I/O in more complicated cases

1. **Compile-time execution**

If the algorithm is a function with callbacks, even with generic programming, then it cannot
propagate `const` because Rust does not have [keyword generics][].

[keyword generics]: https://blog.rust-lang.org/inside-rust/2023/02/23/keyword-generics-progress-report-feb-2023.html

If the algorithm is a coroutine written with [`corophage`][], then it cannot propagate `const`
either.

But, if the coroutine is a handwritten machine state, then it can propagate `const`. For example,
in the below code, the `demo` function is `const`:

```rust
fn main() {
    println!("The answer: {ANSWER}");
}

const ANSWER: u32 = demo();

#[must_use]
pub const fn demo() -> u32 {
    let mut coroutine = compute_answer(10);
    loop {
        coroutine = match coroutine {
            Yield::WantsNumberToMultiply(coroutine) => coroutine.resume(4),
            Yield::WantsNumberToAdd(coroutine) => coroutine.resume(2),
            Yield::Return(number) => break number,
        }
    }
}

pub const fn compute_answer(number: u32) -> Yield {
    Yield::WantsNumberToMultiply(WantsNumberToMultiply(number))
}

#[must_use]
pub enum Yield {
    WantsNumberToMultiply(WantsNumberToMultiply),
    WantsNumberToAdd(WantsNumberToAdd),
    Return(u32),
}

pub struct WantsNumberToMultiply(u32);
pub struct WantsNumberToAdd(u32);

impl WantsNumberToMultiply {
    pub const fn resume(self, number: u32) -> Yield {
        Yield::WantsNumberToAdd(WantsNumberToAdd(self.0 * number))
    }
}

impl WantsNumberToAdd {
    pub const fn resume(self, number: u32) -> Yield {
        Yield::Return(self.0 + number)
    }
}
```

2. **Parallel operations**

Example: fetch data from 2 distinct servers.

If the algorithm is an async function with async callbacks, then the algorithm code can call
`join!` on the futures.

If the algorithm is a coroutine, then it has to yield a pair: the inputs of both fetch operations.
Then the caller code executes the fetch operations and, if both succeed, resumes the coroutine by
sending it a pair: the outputs of both fetch operations.

3. **Ordering constraints**

Example: connect to a remote server, then do operations on it, then disconnect. The disconnection
must happen after the connection. The server operations can happen only after the connection and
before the disconnection.

If the algorithm is a function, then it has a `connect` callback which returns a
`Result<Handle, E>`. `Handle` provides the server operations and a `disconnect` method which
destroys the handle.

In sync Rust, you can call `disconnect` from `Handle::drop`. But, if the algorithm and `disconnect`
are async functions, then you should call `disconnect` yourself. Be careful not to skip the call to
`disconnect` by an early return, for example by propagating errors with the `?` operator.
Rust does not support linear types yet.

If the algorithm is a coroutine which can yield, among other things, a `Connect`, a server
operation description or a `Disconnect`, how to ensure the server operation descriptions are
yielded only between `Connect` and `Disconnect` and what the caller code looks like?

If the coroutine is a handwritten machine state, after yielding a `Connect` and before a
`Disconnect`, the corresponding states have a `resume` method with another return type (`Yield2`
in the below example) which allows yielding a server operation description. The caller code, after
receiving a `Connect`, creates a `Handle` and an inner loop whose `match` branches include handling
the server operation descriptions. When there is a `Disconnect` or a server operation failure, the
caller code disconnects, destroys the `Handle` and goes back to the outer loop. Rust does not
support linear types so be careful not to skip the disconnection, especially in an async context.

The caller code looks like this:

```rust
let mut coroutine = algorithm( ... ); // or `algorithm( ... )?` or `algorithm( ... ).await?`
loop {
    coroutine = match coroutine {
        Yield::HandleSomeIO(coroutine) => {
            ...
            coroutine.resume( ... )
        }
        ... // other branches
        Yield::Connect(coroutine) => {
            ... // connect to the server, create a `Handle`
            let mut coroutine = coroutine.resume();
            let disconnected_state = loop {
                coroutine = match coroutine {
                    Yield2::HandleSomeServerOperation(coroutine) => {
                        ...
                        coroutine.resume( ... )
                    }
                    ... // other branches
                    Yield2::Disconnect(coroutine) => break coroutine, // or Ok(coroutine)
                }
            };
            ... // disconnect and handle error(s)
            disconnected_state.resume() // adapt this if `disconnected_state` is a `Result`
        }
        Yield::Return(value) => break value, // or Ok(value)
    }
}
```

If the algorithm is a coroutine written with [`corophage`][], enforcing ordering constraints does
not seem possible.

## Remarks

I wish Rust had language support to write coroutines more easily, but this is not easy to design
right. The Rust team is working on guaranteed destructors. This is wise to do it before adding more
abstractions.

About coroutines and ordering constraints, some researches are working on combining effect handlers
with linear types.
