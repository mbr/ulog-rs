use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

pub struct LogMsg {

}

pub struct AsyncLogger {
    failures: AtomicUsize,
    sender: SyncSender<LogMsg>,
}

impl AsyncLogger {
    pub fn new(bufsize: usize) -> (AsyncLogger, Receiver<LogMsg>) {
        let (tx, rx) = sync_channel(bufsize);

        (AsyncLogger {
            failures: AtomicUsize::new(0),
            sender: tx,
        },
         rx)
    }

    #[inline]
    pub fn failed(&self) -> bool {
        self.failures() != 0
    }

    #[inline]
    pub fn failures(&self) -> usize {
        self.failures.load(Ordering::Relaxed)
    }
}

fn init() {
    println!("INIT LOGGING");
}

fn main() {
    init();

    println!("Hello, world!");
}
