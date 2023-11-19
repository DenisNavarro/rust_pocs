// Adapted from:
// https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs
// The original code lacks error handling. To limit the differences, I did not fix it.

use std::num::NonZeroUsize;
use std::sync::{mpsc, Mutex};
use std::thread::{Scope, ScopedJoinHandle};

pub struct ThreadPool<'scope> {
    workers: Vec<Worker<'scope>>,
    sender: Option<mpsc::Sender<Job>>, // `Option` for destroying the sender in `ThreadPool::drop`
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl<'scope> ThreadPool<'scope> {
    pub fn new<'env>(
        s: &'scope Scope<'scope, 'env>,
        sender: mpsc::Sender<Job>,
        receiver: &'env Mutex<mpsc::Receiver<Job>>,
        // In the original code, `ThreadPool::new` panicked if the size (thread count) was 0:
        // https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs#L20
        thread_count: NonZeroUsize,
    ) -> ThreadPool<'scope> {
        let thread_count = thread_count.get();
        let mut workers = Vec::with_capacity(thread_count);
        for id in 0..thread_count {
            workers.push(Worker::new(s, id, receiver));
        }
        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap(); // unwrap like in the original code
    }
}

impl<'scope> Drop for ThreadPool<'scope> {
    fn drop(&mut self) {
        drop(self.sender.take());
        for worker in self.workers.drain(..) {
            println!("Shutting down worker {}", worker.id);
            worker.thread.join().unwrap(); // unwrap like in the original code
        }
    }
}

struct Worker<'scope> {
    id: usize,
    // In the original code, the `thread` field was an `Option<thread::JoinHandle<()>>`:
    // https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs#L66
    // Why `Option`? Because `ThreadPool::drop` called `Option::take`:
    // https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs#L57
    // But it was useless. A better solution is to call `Vec::drain`.
    thread: ScopedJoinHandle<'scope, ()>,
}

impl<'scope> Worker<'scope> {
    fn new<'env>(
        s: &'scope Scope<'scope, 'env>,
        id: usize,
        receiver: &'env Mutex<mpsc::Receiver<Job>>,
    ) -> Worker<'scope> {
        // The original code called `std::thread::spawn` instead of `std::thread::Scope::spawn`:
        // https://github.com/rust-lang/book/blob/8d3584f55fa7f70ee699016be7e895d35d0e9b27/listings/ch20-web-server/no-listing-07-final-code/src/lib.rs#L71
        // This change had a lot of consequences in the calling code.
        let thread = s.spawn(move || loop {
            let message = receiver.lock().unwrap().recv(); // unwrap like in the original code
            if let Ok(job) = message {
                println!("Worker {id} got a job; executing.");
                job();
            } else {
                println!("Worker {id} disconnected; shutting down.");
                break;
            }
        });
        Worker { id, thread }
    }
}
