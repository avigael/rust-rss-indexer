use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// Message type to communicate with workers. A JobMsg is either a FnOnce closure or None, which
/// signals the worker to shut down.
type JobMsg = Option<Box<dyn FnOnce() + Send + 'static>>;

/// A ThreadPool should have a sending-end of a mpsc channel (`mpsc::Sender`) and a vector of
/// `JoinHandle`s for the worker threads.
pub struct ThreadPool {
    sender: mpsc::Sender<JobMsg>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl ThreadPool {
    pub fn new(num_workers: usize) -> Self {
        let (sender, receiver): (mpsc::Sender<JobMsg>, mpsc::Receiver<JobMsg>) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(num_workers);

        for _ in 0..num_workers {
            let my_receiver = Arc::clone(&receiver);
            let thread = thread::spawn(move || loop {
                let msg = my_receiver.lock().unwrap().recv().unwrap();
                match msg {
                    Some(job) => job(),
                    None => break,
                }
            });
            workers.push(thread);
        }

        ThreadPool { sender, workers }
    }

    /// Push a new job into the thread pool.
    pub fn execute<F>(&mut self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.sender.send(Some(Box::new(job))).unwrap();
    }
}

impl Drop for ThreadPool {
    /// Clean up the thread pool. Send a kill message (None) to each worker, and join each worker.
    /// This function should only return when all workers have finished.
    fn drop(&mut self) {
        for _ in 0..self.workers.len() {
            self.sender.send(None).unwrap();
        }
        for _ in 0..self.workers.len() {
            self.workers
                .pop()
                .unwrap()
                .join()
                .expect("Failed to join thread");
        }
    }
}
