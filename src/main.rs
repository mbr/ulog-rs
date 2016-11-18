#[macro_use]
extern crate log;
extern crate thread_local;

use log::{LogRecord, LogLevel, LogLevelFilter, LogMetadata};

use std::thread;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};


use thread_local::ThreadLocal;

#[derive(Debug)]
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
            let guard = self.sender.lock().expect("lock poisoned while
                trying to clone new receiver for thread");
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

fn init<F>(bufsize: usize, handle: F)
    where F: Fn(LogMessage) -> (),
          F: Send + 'static
{
    let (logger, rx) = AsyncLogger::new(bufsize);

    thread::spawn(move || {
        loop {
            // FIXME: maybe just exit successfully?
            let msg = rx.recv().expect("closed logging channel");
            handle(msg);
        }
    });

    log::set_logger(move |max_log_level| {
        // FIXME: max_log_level ignored?
        max_log_level.set(LogLevelFilter::Debug);
        Box::new(logger)
    });
}

fn main() {
    init(128, move |msg| println!("Handling your logs: {:?}", msg));
    debug!("Log test");

    loop {}
}
