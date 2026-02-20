//! Handle UNIX process signals with a shared channel.
//!
//! This crate provides a correct, ergonomic approach to UNIX signal handling.
//! The self-pipe trick ensures that the actual signal handler only calls
//! `write(2)` — the only async-signal-safe operation needed. A background
//! thread named `signal-msg` performs all non-trivial work. Multiple
//! independent subscribers are supported via [`Signals::subscribe`].
//!
//! Signal handlers are installed automatically when [`Signals::new`] returns,
//! so it is impossible to forget to activate them.
//!
//! # Example
//!
//! ```no_run
//! use signal_msg::Signals;
//!
//! let signals = Signals::new().expect("failed to create signal handler");
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

/// Global write end of the self-pipe. Set by [`Signals::new`], cleared on drop.
/// Written to only by [`pipe_handler`], which is async-signal-safe.
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

/// OS-level signal handler. Must only call async-signal-safe functions.
/// Writes the signal number as a single byte into the self-pipe.
extern "C" fn pipe_handler(sig: libc::c_int) {
    let fd = WRITE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        // Signal numbers fit in a u8 (POSIX signals are 1–31).
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let byte = sig as u8;
        // SAFETY: `fd` is a valid open file descriptor (the write end of the
        // self-pipe, published by Signals::new before any signal handler is
        // registered). `byte` is a pointer to a live stack variable of the
        // correct size. `libc::write` is async-signal-safe per POSIX.2017 §2.4.3.
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

/// Installs `pipe_handler` for one signal number via `sigaction(2)`.
///
/// # Safety
///
/// `signum` must be a valid, catchable POSIX signal number. `pipe_handler` must
/// remain a valid function pointer for the lifetime of the process (it is a
/// `static extern "C" fn`, so this is always true). `pipe_handler` only calls
/// `write(2)`, which is async-signal-safe per POSIX.2017 §2.4.3.
unsafe fn install_handler(signum: libc::c_int) {
    let mut sa: libc::sigaction = std::mem::zeroed();
    sa.sa_sigaction = pipe_handler as *const () as libc::sighandler_t;
    sa.sa_flags = libc::SA_RESTART;
    libc::sigaction(signum, &sa, std::ptr::null_mut());
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
///
/// `SignalError` implements `Clone` and `PartialEq`; for the
/// [`OsError`][SignalError::OsError] variant only the
/// [`io::ErrorKind`][std::io::ErrorKind] is compared and cloned, since
/// [`io::Error`][std::io::Error] itself is neither `Clone` nor `PartialEq`.
#[derive(Debug)]
pub enum SignalError {
    /// The channel is disconnected (the paired [`Signals`] handle was dropped).
    Disconnected,
    /// [`Signals::new`] was called while another `Signals` instance is active.
    AlreadyInitialized,
    /// An OS-level operation failed during signal channel setup.
    OsError(std::io::Error),
}

impl Clone for SignalError {
    fn clone(&self) -> Self {
        match self {
            Self::Disconnected => Self::Disconnected,
            Self::AlreadyInitialized => Self::AlreadyInitialized,
            // Preserve the error kind; the OS message is lost on clone.
            Self::OsError(e) => Self::OsError(std::io::Error::from(e.kind())),
        }
    }
}

impl PartialEq for SignalError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Disconnected, Self::Disconnected) => true,
            (Self::AlreadyInitialized, Self::AlreadyInitialized) => true,
            (Self::OsError(a), Self::OsError(b)) => a.kind() == b.kind(),
            _ => false,
        }
    }
}

impl Eq for SignalError {}

impl std::fmt::Display for SignalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalError::Disconnected => f.write_str("signal channel disconnected"),
            SignalError::AlreadyInitialized => {
                f.write_str("a Signals instance is already active")
            }
            SignalError::OsError(e) => write!(f, "OS error during signal channel setup: {e}"),
        }
    }
}

impl std::error::Error for SignalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SignalError::OsError(e) => Some(e),
            _ => None,
        }
    }
}

type Senders = Arc<Mutex<Vec<mpsc::Sender<Signal>>>>;

#[derive(Debug)]
struct SignalsInner {
    write_fd: RawFd,
    senders: Senders,
}

impl Drop for SignalsInner {
    fn drop(&mut self) {
        // Closing write_fd causes the background thread's read() to return 0
        // (EOF), signalling it to exit cleanly.
        // SAFETY: `write_fd` is a valid open file descriptor owned exclusively
        // by this struct. No other code closes it while SignalsInner is alive.
        unsafe { libc::close(self.write_fd) };
        WRITE_FD.store(-1, Ordering::Relaxed);
        INITIALIZED.store(false, Ordering::Relaxed);
    }
}

/// A handle for subscribing to OS signals delivered through a shared channel.
///
/// `Signals` is cheaply cloneable (backed by an [`Arc`]). Only one `Signals`
/// instance may be active at a time per process; calling [`Signals::new`] while
/// another instance exists returns [`SignalError::AlreadyInitialized`].
///
/// OS-level signal handlers are installed automatically by [`Signals::new`],
/// so signals are delivered from the moment the value is returned. Call
/// [`subscribe`][Signals::subscribe] to obtain a [`Receiver`].
///
/// Dropping the last clone releases all resources including the background
/// thread.
///
/// # Example
///
/// ```no_run
/// use signal_msg::Signals;
///
/// let signals = Signals::new().expect("failed to create signal handler");
/// let receiver = signals.subscribe();
/// match receiver.listen() {
///     Ok(sig) => println!("received: {}", sig),
///     Err(e)  => eprintln!("error: {}", e),
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Signals(Arc<SignalsInner>);

impl Signals {
    /// Creates a new signal channel and installs OS-level signal handlers.
    ///
    /// Allocates a self-pipe, spawns a background dispatch thread named
    /// `signal-msg`, and registers handlers for all supported signals. Call
    /// [`subscribe`][Signals::subscribe] to obtain a [`Receiver`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let signals = signal_msg::Signals::new().expect("signal setup failed");
    /// let receiver = signals.subscribe();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SignalError::AlreadyInitialized`] if another `Signals` instance
    /// is already active. Returns [`SignalError::OsError`] if an OS-level
    /// operation fails (pipe creation, `fcntl`, or thread spawn).
    pub fn new() -> Result<Self, SignalError> {
        if INITIALIZED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(SignalError::AlreadyInitialized);
        }
        Self::try_init().map_err(|e| {
            INITIALIZED.store(false, Ordering::Relaxed);
            e
        })
    }

    fn try_init() -> Result<Self, SignalError> {
        let mut fds = [0i32; 2];
        // SAFETY: `fds` is a valid mutable pointer to a 2-element `c_int`
        // array, satisfying the requirements of `pipe(2)`.
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Err(SignalError::OsError(std::io::Error::last_os_error()));
        }
        let read_fd = fds[0];
        let write_fd = fds[1];

        // SAFETY: `write_fd` and `read_fd` are valid open file descriptors
        // just created by `pipe(2)` above.
        unsafe {
            let fl = libc::fcntl(write_fd, libc::F_GETFL);
            if fl == -1
                || libc::fcntl(write_fd, libc::F_SETFL, fl | libc::O_NONBLOCK) == -1
                || libc::fcntl(write_fd, libc::F_SETFD, libc::FD_CLOEXEC) == -1
                || libc::fcntl(read_fd, libc::F_SETFD, libc::FD_CLOEXEC) == -1
            {
                let e = std::io::Error::last_os_error();
                libc::close(write_fd);
                libc::close(read_fd);
                return Err(SignalError::OsError(e));
            }
        }

        // Publish write_fd before the thread starts so pipe_handler can use it.
        WRITE_FD.store(write_fd, Ordering::Relaxed);

        let senders: Senders = Arc::new(Mutex::new(Vec::new()));
        let thread_senders = Arc::clone(&senders);

        // Background thread: reads signal bytes from the pipe and fans them out
        // to all registered senders. Exits when the write end is closed (EOF).
        std::thread::Builder::new()
            .name("signal-msg".into())
            .spawn(move || {
                let mut buf = [0u8; 64];
                loop {
                    // SAFETY: `read_fd` is a valid open file descriptor owned
                    // exclusively by this thread. `buf` is valid for 64 bytes.
                    let n = unsafe {
                        libc::read(read_fd, buf.as_mut_ptr().cast::<libc::c_void>(), 64)
                    };
                    if n <= 0 {
                        break; // EOF (write end closed) or unrecoverable error
                    }
                    #[allow(clippy::cast_sign_loss)]
                    let received = &buf[..n as usize];
                    let mut locked = thread_senders.lock().unwrap_or_else(|p| p.into_inner());
                    for &byte in received {
                        if let Some(sig) = from_signum(byte) {
                            locked.retain(|s| s.send(sig).is_ok());
                        }
                    }
                }
                // SAFETY: `read_fd` is exclusively owned by this thread and
                // has not been closed by any other code.
                unsafe { libc::close(read_fd) };
            })
            .map_err(|e| {
                // Thread spawn failed; clean up fds before propagating the error.
                // SAFETY: `write_fd` and `read_fd` are valid open file descriptors.
                unsafe {
                    libc::close(write_fd);
                    libc::close(read_fd);
                }
                WRITE_FD.store(-1, Ordering::Relaxed);
                SignalError::OsError(e)
            })?;

        // Install OS-level handlers for all supported signals. This is done
        // after the thread is running so no signals can be lost.
        for &signum in HANDLED_SIGNALS {
            // SAFETY: `signum` is a valid catchable signal number from
            // `HANDLED_SIGNALS` and `pipe_handler` is async-signal-safe.
            unsafe { install_handler(signum) };
        }

        Ok(Signals(Arc::new(SignalsInner { write_fd, senders })))
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
    /// let r1 = signals.subscribe();
    /// let r2 = signals.subscribe();
    /// // r1 and r2 each receive independent copies of every signal.
    /// # let _ = (r1, r2);
    /// ```
    #[must_use]
    pub fn subscribe(&self) -> Receiver {
        let (tx, rx) = mpsc::channel();
        self.0.senders.lock().unwrap_or_else(|p| p.into_inner()).push(tx);
        Receiver(rx)
    }
}

/// Receives UNIX signals forwarded through a [`Signals`] channel.
///
/// Obtained via [`Signals::subscribe`]. Use [`listen`][Receiver::listen] to
/// block until a signal arrives, or [`try_listen`][Receiver::try_listen] to
/// poll without blocking.
#[derive(Debug)]
pub struct Receiver(mpsc::Receiver<Signal>);

impl Receiver {
    /// Blocks until a signal is received and returns it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use signal_msg::Signals;
    ///
    /// let signals = Signals::new().expect("signal setup failed");
    /// let receiver = signals.subscribe();
    /// match receiver.listen() {
    ///     Ok(sig) => println!("received: {}", sig),
    ///     Err(e)  => eprintln!("channel closed: {}", e),
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SignalError::Disconnected`] if the backing [`Signals`] handle
    /// has been dropped and no further signals can arrive.
    pub fn listen(&self) -> Result<Signal, SignalError> {
        self.0.recv().map_err(|_| SignalError::Disconnected)
    }

    /// Returns the next signal if one is immediately available, or `None` if
    /// the channel is currently empty.
    ///
    /// Unlike [`listen`][Receiver::listen], this method never blocks.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use signal_msg::Signals;
    ///
    /// let signals = Signals::new().expect("signal setup failed");
    /// let receiver = signals.subscribe();
    /// match receiver.try_listen() {
    ///     Ok(Some(sig)) => println!("got signal: {}", sig),
    ///     Ok(None)      => println!("no signal pending"),
    ///     Err(e)        => eprintln!("channel closed: {}", e),
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SignalError::Disconnected`] if the backing [`Signals`] handle
    /// has been dropped and no further signals can arrive.
    pub fn try_listen(&self) -> Result<Option<Signal>, SignalError> {
        match self.0.try_recv() {
            Ok(sig) => Ok(Some(sig)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(SignalError::Disconnected),
        }
    }
}
