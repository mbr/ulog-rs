//! ulog: Flexible asynchronous logging
//!
//! ## Features:
//!
//! * Asynchronous: `ulog` sends all log messages through a channel to another
//!   thread as soon as possible, trying to minimize the time the logging
//!   thread spends processing the log.
//! * Standard `log`: `ulog` uses the de-facto standard logging facade of
//!   [log](https://crates.io/crates/log), allowing access to familiar
//!   features.
//! * Flexible: `ulog` is small but flexible, allowing different use cases
//!   other than standard file or stream logging. Log handling is done by
//!   passing a closure that is executed in a different thread.
//!
//! ## Intended use and example
//!
//! `ulog` is intended for use on embedded Linux applications that are
//! multithreaded and require a single thread to provide hard real-time
//! guarantees. Logs made using `ulog` are sent through a [synchronous
//! channel](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html)
//! before being processed, if logging cannot be completed in constant time,
//! the message is dropped and an error flag is set.
//!
//! ### Example use case
//!
//! Consider an application controlling a piece of hardware that has moderate
//! real-time guarantees (in the 10s of milliseconds). Allocations and other
//! execution will usually run just fine, however writing logs to disk or
//! stderr can occasionally slow down the process by hundres of millliseconds
//! under some conditions.
//!
//! To avoid this, `ulog` can be used to offloaded heavy I/O into a separate
//! thread, if enough execution threads are available and scheduling is setup
//! correctly, this will guarantee that logging never takes longer than a
//! string format and a few small allocations.

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

/// A log entry
///
/// The `LogMessage` struct copies most interesting fields from a
/// `log::LogRecord`, allowing owned versions of these to be passed around.
///
/// Any time a `LogMessage` is constructed, the following allocations need to
/// be made:
///
/// * Four pointer-sized fields (`level`, as well as three inside `location`)
/// * An owned copy of `target`
/// * The result of formatting the message (see `LogRecord.args()`).
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

/// Asynchronous logger
///
/// The very first time a message is logged on a channel in a thread, a copy of
/// the sending end of the channel is made, requiring a single mutex lock and
/// clone. For further calls, this sending end is cached as a thread local.
///
/// ## Panics
///
/// Logging may panic in the unlikely event the lock becomes poisoned upon
/// logging from a new thread.
pub struct AsyncLogger {
    failures: AtomicUsize,

    // sender is only used once to clone for each thread
    sender: Mutex<SyncSender<LogMessage>>,

    // use thread-local storage for local instance
    local_sender: thread_local::ThreadLocal<SyncSender<LogMessage>>,
}

impl AsyncLogger {
    /// Creates a new logger.
    ///
    /// `bufsize` denotes the queue size before messages are
    /// dropped without being handled.
    pub fn new(bufsize: usize) -> (AsyncLogger, Receiver<LogMessage>) {
        let (tx, rx) = sync_channel(bufsize);

        (AsyncLogger {
            failures: AtomicUsize::new(0),
            sender: Mutex::new(tx),
            local_sender: ThreadLocal::new(),
        },
         rx)
    }

    /// Number of dropped log messages.
    #[inline]
    pub fn failures(&self) -> usize {
        self.failures.load(Ordering::Relaxed)
    }

    #[inline]
    fn sender(&self) -> &SyncSender<LogMessage> {
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

/// Initialize ulog logging.
///
/// Creates a new `AsyncLogger` with a buffered channel of size `bufsize` and
/// starts a handler thread that will call `handle` for every `LogMessage`
/// received.
pub fn init<F>(bufsize: usize, handle: F) -> Result<(), log::SetLoggerError>
    where F: Fn(LogMessage) -> (),
          F: Send + 'static
{
    let (logger, rx) = AsyncLogger::new(bufsize);

    thread::spawn(move || {
        loop {
            // FIXME: maybe just exit successfully?
            let msg = match rx.recv() {
                Ok(m) => m,
                Err(_) => break,
            };
            handle(msg);
        }
    });

    log::set_logger(move |max_log_level| {
        // FIXME: max_log_level ignored?
        max_log_level.set(LogLevelFilter::Debug);
        Box::new(logger)
    })
}

/// Initialize ulog loggin with default `stderr`-based logging.
///
/// Sets a buffer size of 128 log entries.
///
/// ## Panics
/// Will panic if stderr is unavailable for writing.
pub fn init_stderr() -> Result<(), log::SetLoggerError> {
    init(128, move |msg| {
        writeln!(&mut std::io::stderr(), "{}", msg).expect("Error writing
            log to stderr");
    })
}
