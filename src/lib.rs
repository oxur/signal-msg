//! Handle UNIX process signals with a shared channel.
//!
//! This crate provides a correct, ergonomic approach to UNIX signal handling.
//! The self-pipe trick ensures that the actual signal handler only calls
//! `write(2)` — the only async-signal-safe operation needed. A background
//! thread performs all non-trivial work. Multiple independent subscribers are
//! supported via [`Signals::subscribe`].
//!
//! # Example
//!
//! ```no_run
//! use signal_msg::Signals;
//!
//! let signals = Signals::new().expect("failed to create signal handler");
//! signals.prepare();
//! let receiver = signals.subscribe();
//! println!("Waiting for a signal...");
//! match receiver.listen() {
//!     Ok(sig) => println!("received: {}", sig),
//!     Err(e)  => eprintln!("channel error: {}", e),
//! }
//! ```

use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{mpsc, Arc, Mutex};

/// Global write end of the self-pipe.  Set by [`Signals::new`], cleared on drop.
/// The signal handler reads this and writes a single byte per signal.
static WRITE_FD: AtomicI32 = AtomicI32::new(-1);

/// Guards against creating multiple [`Signals`] instances simultaneously.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

const HANDLED_SIGNALS: &[libc::c_int] = &[
    libc::SIGHUP,
    libc::SIGINT,
    libc::SIGILL,
    libc::SIGABRT,
    libc::SIGFPE,
    libc::SIGPIPE,
    libc::SIGALRM,
    libc::SIGTERM,
];

/// OS-level signal handler.  Must only call async-signal-safe functions.
/// Writes the signal number as a single byte into the self-pipe.
extern "C" fn pipe_handler(sig: libc::c_int) {
    let fd = WRITE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        // Signal numbers fit in a u8 (POSIX signals are 1–31).
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let byte = sig as u8;
        unsafe {
            libc::write(fd, std::ptr::addr_of!(byte).cast::<libc::c_void>(), 1);
        }
    }
}

fn from_signum(n: u8) -> Option<Signal> {
    match libc::c_int::from(n) {
        libc::SIGHUP => Some(Signal::Hup),
        libc::SIGINT => Some(Signal::Int),
        libc::SIGILL => Some(Signal::Ill),
        libc::SIGABRT => Some(Signal::Abrt),
        libc::SIGFPE => Some(Signal::Fpe),
        libc::SIGPIPE => Some(Signal::Pipe),
        libc::SIGALRM => Some(Signal::Alrm),
        libc::SIGTERM => Some(Signal::Term),
        _ => None,
    }
}

/// A UNIX signal that can be received through a [`Signals`] channel.
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
            Signal::Hup => "SIGHUP",
            Signal::Int => "SIGINT",
            Signal::Ill => "SIGILL",
            Signal::Abrt => "SIGABRT",
            Signal::Fpe => "SIGFPE",
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
    /// The channel is disconnected (the paired [`Signals`] handle was dropped).
    Disconnected,
    /// [`Signals::new`] was called while another `Signals` instance is active.
    AlreadyInitialized,
    /// An OS-level operation failed during signal channel setup.
    OsError,
}

impl std::fmt::Display for SignalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalError::Disconnected => f.write_str("signal channel disconnected"),
            SignalError::AlreadyInitialized => {
                f.write_str("a Signals instance is already active")
            }
            SignalError::OsError => f.write_str("OS error during signal channel setup"),
        }
    }
}

impl std::error::Error for SignalError {}

type Senders = Arc<Mutex<Vec<mpsc::Sender<Signal>>>>;

struct SignalsInner {
    write_fd: RawFd,
    senders: Senders,
}

impl Drop for SignalsInner {
    fn drop(&mut self) {
        // Closing write_fd causes the background thread's read() to return 0
        // (EOF), which signals the thread to exit cleanly.
        unsafe { libc::close(self.write_fd) };
        WRITE_FD.store(-1, Ordering::Relaxed);
        INITIALIZED.store(false, Ordering::Relaxed);
    }
}

/// A handle for registering OS signal handlers and creating signal receivers.
///
/// `Signals` is cheaply cloneable (backed by an [`Arc`]). Only one `Signals`
/// instance may be active at a time per process; calling [`Signals::new`] while
/// another instance exists returns [`SignalError::AlreadyInitialized`].
///
/// Dropping the last clone of a `Signals` handle de-registers the OS handlers
/// and releases all resources including the background thread.
///
/// # Example
///
/// ```no_run
/// use signal_msg::Signals;
///
/// let signals = Signals::new().expect("failed to create signal handler");
/// signals.prepare();
/// let receiver = signals.subscribe();
/// match receiver.listen() {
///     Ok(sig) => println!("received: {}", sig),
///     Err(e)  => eprintln!("error: {}", e),
/// }
/// ```
#[derive(Clone)]
pub struct Signals(Arc<SignalsInner>);

impl Signals {
    /// Creates a new signal channel.
    ///
    /// Allocates a self-pipe and spawns a background dispatch thread.
    /// After creation, call [`prepare`][Signals::prepare] to register OS-level
    /// signal handlers, then [`subscribe`][Signals::subscribe] to obtain a
    /// [`Receiver`].
    ///
    /// # Errors
    ///
    /// Returns [`SignalError::AlreadyInitialized`] if another `Signals` instance
    /// is already active. Returns [`SignalError::OsError`] if the OS pipe
    /// allocation fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let signals = signal_msg::Signals::new().expect("signal setup failed");
    /// ```
    pub fn new() -> Result<Self, SignalError> {
        if INITIALIZED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(SignalError::AlreadyInitialized);
        }

        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            INITIALIZED.store(false, Ordering::SeqCst);
            return Err(SignalError::OsError);
        }
        let read_fd = fds[0];
        let write_fd = fds[1];

        unsafe {
            // Write end: non-blocking so the handler never stalls + close-on-exec.
            let fl = libc::fcntl(write_fd, libc::F_GETFL);
            libc::fcntl(write_fd, libc::F_SETFL, fl | libc::O_NONBLOCK);
            libc::fcntl(write_fd, libc::F_SETFD, libc::FD_CLOEXEC);
            // Read end: close-on-exec (used only by the background thread).
            libc::fcntl(read_fd, libc::F_SETFD, libc::FD_CLOEXEC);
        }

        WRITE_FD.store(write_fd, Ordering::Relaxed);

        let senders: Senders = Arc::new(Mutex::new(Vec::new()));
        let thread_senders = Arc::clone(&senders);

        // Background thread: reads signal bytes from the pipe, fans out to
        // all registered senders.  Exits when the write end is closed (EOF).
        std::thread::spawn(move || {
            let mut buf = [0u8; 64];
            loop {
                let n =
                    unsafe { libc::read(read_fd, buf.as_mut_ptr().cast::<libc::c_void>(), 64) };
                if n <= 0 {
                    break;
                }
                #[allow(clippy::cast_sign_loss)]
                let received = &buf[..n as usize];
                let mut locked = thread_senders.lock().unwrap();
                for &byte in received {
                    if let Some(sig) = from_signum(byte) {
                        locked.retain(|s| s.send(sig).is_ok());
                    }
                }
            }
            unsafe { libc::close(read_fd) };
        });

        Ok(Signals(Arc::new(SignalsInner { write_fd, senders })))
    }

    /// Registers OS-level handlers for all supported signals.
    ///
    /// After this call, any supported signal received by the process is
    /// forwarded to all active [`Receiver`]s obtained via
    /// [`subscribe`][Signals::subscribe].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let signals = signal_msg::Signals::new().expect("signal setup failed");
    /// signals.prepare();
    /// ```
    pub fn prepare(&self) {
        for &signum in HANDLED_SIGNALS {
            unsafe {
                let mut sa: libc::sigaction = std::mem::zeroed();
                sa.sa_sigaction = pipe_handler as *const () as libc::sighandler_t;
                sa.sa_flags = libc::SA_RESTART;
                libc::sigaction(signum, &sa, std::ptr::null_mut());
            }
        }
    }

    /// Returns a new [`Receiver`] that will receive all subsequent signals.
    ///
    /// Multiple independent receivers can be created from the same `Signals`
    /// handle; each receives its own copy of every delivered signal.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use signal_msg::Signals;
    ///
    /// let signals = Signals::new().expect("signal setup failed");
    /// signals.prepare();
    /// let r1 = signals.subscribe();
    /// let r2 = signals.subscribe();
    /// // r1 and r2 each receive independent copies of every signal.
    /// # let _ = (r1, r2);
    /// ```
    #[must_use]
    pub fn subscribe(&self) -> Receiver {
        let (tx, rx) = mpsc::channel();
        self.0.senders.lock().unwrap().push(tx);
        Receiver(rx)
    }
}

/// Receives UNIX signals forwarded through a [`Signals`] channel.
///
/// Obtained via [`Signals::subscribe`]. Blocks on [`listen`][Receiver::listen]
/// until a signal arrives.
pub struct Receiver(mpsc::Receiver<Signal>);

impl Receiver {
    /// Blocks until a signal is received and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`SignalError::Disconnected`] if the backing [`Signals`] handle
    /// has been dropped and no further signals can arrive.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use signal_msg::Signals;
    ///
    /// let signals = Signals::new().expect("signal setup failed");
    /// signals.prepare();
    /// let receiver = signals.subscribe();
    /// match receiver.listen() {
    ///     Ok(sig) => println!("received: {}", sig),
    ///     Err(e)  => eprintln!("channel closed: {}", e),
    /// }
    /// ```
    pub fn listen(&self) -> Result<Signal, SignalError> {
        self.0.recv().map_err(|_| SignalError::Disconnected)
    }
}
