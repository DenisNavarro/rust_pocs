// Adapted from:
// https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/main.rs
// The original code lack error handling. I don't know yet if I will fix that too in this POC.

#![warn(clippy::nursery, clippy::pedantic)]
#![allow(clippy::unused_io_amount)]

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::num::NonZeroUsize;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;

use structured_concurrency::{ThreadCount, ThreadPool};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap(); // unwrap like in the original code
    let (sender, receiver) = mpsc::channel();

    // Instead of an Arc<Mutex<Receiver<Job>>>, we create a
    // Mutex<Receiver<Job>> before the thread scope.
    // In the original code, it was created inside `ThreadPool::new`:
    // https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs#L26
    let receiver = Mutex::new(receiver);

    thread::scope(|s| {
        let thread_count = ThreadCount(NonZeroUsize::new(4).unwrap());
        let pool = ThreadPool::new(s, sender, &receiver, thread_count);
        for stream in listener.incoming().take(2) {
            let stream = stream.unwrap(); // unwrap like in the original code
            pool.execute(|| {
                handle_connection(stream);
            });
        }
    });
    println!("Shutting down.");
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];

    // This line triggers the warning: clippy::unused_io_amount.
    // It was like this in the original code:
    // https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/main.rs#L26
    stream.read(&mut buffer).unwrap(); // unwrap like in the original code

    let get = b"GET / HTTP/1.1\r\n";
    let sleep = b"GET /sleep HTTP/1.1\r\n";
    let (status_line, filename) = if buffer.starts_with(get) {
        ("HTTP/1.1 200 OK", "hello.html")
    } else if buffer.starts_with(sleep) {
        thread::sleep(Duration::from_secs(5));
        ("HTTP/1.1 200 OK", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };
    let contents = fs::read_to_string(filename).unwrap(); // unwrap like in the original code
    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line,
        contents.len(),
        contents
    );
    stream.write_all(response.as_bytes()).unwrap(); // unwrap like in the original code
    stream.flush().unwrap(); // unwrap like in the original code
}
