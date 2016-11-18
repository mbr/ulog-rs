extern crate log;
extern crate thread_local;

use log::{LogRecord, LogLevel, LogMetadata};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};


use thread_local::ThreadLocal;

// thread_local!(static FOO: usize = 123);

pub struct LogMessage {

}

impl LogMessage {
    fn new() -> LogMessage {
        LogMessage {}
    }
}

pub struct AsyncLogger {
    failures: AtomicUsize,

    // sender is only used once to clone for each thread
    sender: Mutex<SyncSender<LogMessage>>,

    // use thread-local storage for local instance
    local_sender: thread_local::ThreadLocal<SyncSender<LogMessage>>,
}

impl AsyncLogger {
    pub fn new(bufsize: usize) -> (AsyncLogger, Receiver<LogMessage>) {
        let (tx, rx) = sync_channel(bufsize);

        (AsyncLogger {
            failures: AtomicUsize::new(0),
            sender: Mutex::new(tx),
            local_sender: ThreadLocal::new(),
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

    #[inline]
    pub fn sender(&self) -> &SyncSender<LogMessage> {
        // check if we already have a suitable sender, if not, we need to clone
        // it

        self.local_sender.get_or(|| {
            let guard = self.sender.lock().unwrap();
            let local_tx = guard.clone();
            Box::new(local_tx)
        })
    }
}

impl log::Log for AsyncLogger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        // FIXME
        metadata.level() <= LogLevel::Debug
    }

    fn log(&self, record: &LogRecord) {
        let msg = LogMessage::new();
        if let Err(_) = self.sender().try_send(msg) {
            // failed to send log message; increase failure count
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn init() {
    println!("INIT LOGGING");
}

fn main() {
    init();

    println!("Hello, world!");
}
