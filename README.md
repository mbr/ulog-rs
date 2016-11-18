ulog
====

A small logging library (not only) for hard real-time logging. Key features:

* Asynchronous: `ulog` sends all log messages through a channel to another thread as soon as possible, trying to minimize the time the logging thread spends processing the log.
* Standard `log`: `ulog` uses the de-facto standard logging facade of [log](https://crates.io/crates/log), allowing access to familiar features.
* Flexible: `ulog` is small but flexible, allowing different use cases other than standard file or stream logging. Log handling is done by passing a closure that is executed in a different thread.


Real-time
---------

`ulog` is intended for use on embedded Linux applications that are multithreaded and require a single thread to be have some hard real-time guarantess. Logs made using `ulog` are sent through a [synchronous channel](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html) before being processed, if logging cannot be completed in constant time, the message is dropped and an error flag is set.

However, allocations have to be made before sending the log entry for asynchronouos processing, to copy fields of a log message, see the `LogMessage` docs for details.


Other logging crates
--------------------

There are a *lot* of other logging crates that offer similar functionality, the closest one in focus is [fastlog](https://crates.io/crates/fastlog). While `fastlog` shares the focus on asynchronous logging, it is unfortunately limited to predefined ways of logging that are mostly files and stdout/-err.
