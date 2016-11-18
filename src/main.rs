#[macro_use]
extern crate log;
extern crate thread_local;

use log::{LogRecord, LogLocation, LogLevel, LogLevelFilter, LogMetadata};

use std::{fmt, thread};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};


use thread_local::ThreadLocal;

#[derive(Debug)]
pub struct LogMessage {
    pub level: LogLevel,
    pub target: String,
    pub msg: String,
    pub location: LogLocation,
}

impl fmt::Display for LogMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.level, self.target, self.msg)
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
        // NOTE: allocations happen here
        let msg = LogMessage {
            level: record.metadata().level(),
            target: record.metadata().target().to_owned(),
            msg: format!("{}", record.args()),
            location: record.location().clone(),
        };
        if let Err(_) = self.sender().try_send(msg) {
            // failed to send log message; increase failure count
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn init<F>(bufsize: usize, handle: F) -> Result<(), log::SetLoggerError>
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
    })
}

fn init_stderr() -> Result<(), log::SetLoggerError> {
    init(128, move |msg| {
        writeln!(&mut std::io::stderr(), "{}", msg).expect("Error writing
            log to stderr");
    })
}

fn main() {
    init_stderr().unwrap();


    debug!("Log test");
    loop {
    }
}
