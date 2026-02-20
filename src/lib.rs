//! Handle UNIX process signals with a shared channel.
//!
//! This library provides a simple, message-based approach to signal handling.
//! Instead of putting logic inside signal-handler closures (which cannot easily
//! share mutable state), `signal-msg` lets you receive signals on an
//! [`mpsc::Receiver`], just like any other message.
//!
//! # Example
//!
//! ```no_run
//! use signal_msg::{SignalReceiver, SignalSender};
//!
//! let (sender, receiver) = signal_msg::new();
//! sender.prepare_signals();
//! println!("Waiting for a signal...");
//! match receiver.listen() {
//!     Ok(sig) => println!("received: {}", sig),
//!     Err(e)  => eprintln!("channel error: {}", e),
//! }
//! ```

use std::sync::mpsc;

/// A UNIX signal that can be received through a [`signal_msg`] channel.
///
/// Signals that cannot be caught (`SIGKILL`), that generate core dumps by
/// default (`SIGQUIT`), or that indicate unrecoverable program faults
/// (`SIGSEGV`) are intentionally excluded.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Signal {
    /// `SIGHUP` — terminal hang-up or controlling process died.
    Hup,
    /// `SIGINT` — interactive interrupt (typically Ctrl-C).
    Int,
    /// `SIGILL` — illegal CPU instruction.
    Ill,
    /// `SIGABRT` — process abort.
    Abrt,
    /// `SIGFPE` — floating-point exception.
    Fpe,
    /// `SIGPIPE` — write to a broken pipe.
    Pipe,
    /// `SIGALRM` — alarm-clock timer expired.
    Alrm,
    /// `SIGTERM` — polite termination request.
    Term,
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Signal::Hup  => "SIGHUP",
            Signal::Int  => "SIGINT",
            Signal::Ill  => "SIGILL",
            Signal::Abrt => "SIGABRT",
            Signal::Fpe  => "SIGFPE",
            Signal::Pipe => "SIGPIPE",
            Signal::Alrm => "SIGALRM",
            Signal::Term => "SIGTERM",
        };
        f.write_str(name)
    }
}

/// An error produced by signal channel operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalError {
    /// The channel is disconnected (the sender or receiver was dropped).
    Disconnected,
}

impl std::fmt::Display for SignalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalError::Disconnected => f.write_str("signal channel disconnected"),
        }
    }
}

impl std::error::Error for SignalError {}

fn handled_signals() -> Vec<simple_signal::Signal> {
    vec![
        simple_signal::Signal::Hup,
        simple_signal::Signal::Int,
        simple_signal::Signal::Ill,
        simple_signal::Signal::Abrt,
        simple_signal::Signal::Fpe,
        simple_signal::Signal::Pipe,
        simple_signal::Signal::Alrm,
        simple_signal::Signal::Term,
    ]
}

fn from_simple_signal(sig: &simple_signal::Signal) -> Option<Signal> {
    match sig {
        simple_signal::Signal::Hup  => Some(Signal::Hup),
        simple_signal::Signal::Int  => Some(Signal::Int),
        simple_signal::Signal::Ill  => Some(Signal::Ill),
        simple_signal::Signal::Abrt => Some(Signal::Abrt),
        simple_signal::Signal::Fpe  => Some(Signal::Fpe),
        simple_signal::Signal::Pipe => Some(Signal::Pipe),
        simple_signal::Signal::Alrm => Some(Signal::Alrm),
        simple_signal::Signal::Term => Some(Signal::Term),
        _ => None,
    }
}

/// Registers signal handlers and forwards signals into a channel.
pub trait SignalSender {
    /// Sets up OS-level handlers for all supported signals.
    ///
    /// After calling this, any handled signal received by the process is sent
    /// through the channel to the paired [`SignalReceiver`]. If the receiver
    /// has already been dropped, the signal is silently discarded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use signal_msg::SignalSender;
    ///
    /// let (sender, _receiver) = signal_msg::new();
    /// sender.prepare_signals();
    /// ```
    fn prepare_signals(&self);
}

impl SignalSender for mpsc::Sender<Signal> {
    fn prepare_signals(&self) {
        let s = self.clone();
        simple_signal::set_handler(&handled_signals(), move |signals| {
            for sig in signals {
                if let Some(signal) = from_simple_signal(sig) {
                    let _ = s.send(signal);
                }
            }
        });
    }
}

/// Receives UNIX signals forwarded through a channel.
pub trait SignalReceiver {
    /// Blocks until a signal is received, then returns it.
    ///
    /// # Errors
    ///
    /// Returns [`SignalError::Disconnected`] if the sending side of the
    /// channel has been dropped and no further signals can be received.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use signal_msg::{SignalReceiver, SignalSender};
    ///
    /// let (sender, receiver) = signal_msg::new();
    /// sender.prepare_signals();
    /// match receiver.listen() {
    ///     Ok(sig) => println!("received: {}", sig),
    ///     Err(e)  => eprintln!("channel closed: {}", e),
    /// }
    /// ```
    fn listen(&self) -> Result<Signal, SignalError>;
}

impl SignalReceiver for mpsc::Receiver<Signal> {
    fn listen(&self) -> Result<Signal, SignalError> {
        self.recv().map_err(|_| SignalError::Disconnected)
    }
}

/// Creates a linked sender/receiver pair for signal delivery.
///
/// Call [`SignalSender::prepare_signals`] on the sender to register OS-level
/// signal handlers, then use [`SignalReceiver::listen`] on the receiver to
/// block until a signal arrives.
///
/// # Examples
///
/// ```no_run
/// use signal_msg::{SignalReceiver, SignalSender};
///
/// let (sender, receiver) = signal_msg::new();
/// sender.prepare_signals();
/// println!("Waiting for a signal...");
/// match receiver.listen() {
///     Ok(sig) => println!("received: {}", sig),
///     Err(e)  => eprintln!("error: {}", e),
/// }
/// ```
pub fn new() -> (mpsc::Sender<Signal>, mpsc::Receiver<Signal>) {
    mpsc::channel()
}
