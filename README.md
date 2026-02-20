# signal-msg

[![][build-badge]][build]
[![][crate-badge]][crate]
[![][tag-badge]][tag]
[![][docs-badge]][docs]

*UNIX signal handling as an iterator over a shared channel*

[![][logo]][logo-large]

## About

This project makes UNIX signal handling simple: just listen for signals on a
channel. Instead of putting logic inside signal-handler closures (which are
restricted to async-signal-safe operations and cannot easily share state),
`signal-msg` delivers signals as messages to one or more [`Receiver`]s.

Internally the library uses the self-pipe trick — the OS-level handler writes
a single byte into a pipe (the only async-signal-safe work it does), and a
background thread reads from the pipe and fans the signal out to all
subscribers. A more feature-rich solution is available via the
[signal-hook](https://github.com/vorner/signal-hook) library.

Similar functionality to signal-msg is provided by the
[signal-notify](https://crates.io/crates/signal-notify) and
[chan-signal](https://crates.io/crates/chan-signal) libraries (note, though,
that the latter is deprecated and recommends exploring both `signal-hook` and [crossbeam-channel](https://github.com/crossbeam-rs/crossbeam/tree/master/crossbeam-channel)).

## Learning Experiment

This project was born in early 2020, when Rust was a new and exciting language to explore. Coming
from backgrounds in Erlang, Clojure, and recently Go — languages where message-passing and channels are
first-class citizens — the idea of wrapping UNIX signal handling in a familiar channel-based API
felt like a natural first real-world project. It was a chance to poke at `std::sync::mpsc`, write
some traits, publish a crate, and generally get a feel for how Rust thinks about ownership,
concurrency, and interfacing with the OS.

Fast-forward to 2026: coming back to the project made it immediately clear that the library isn't
really necessary. The ecosystem has matured beautifully — see the [About](#about) section above for
better-maintained, more feature-complete alternatives worth exploring. But rather than shelving it,
it seemed like a much better idea to double down on the learning angle: bring the code up to
current Rust standards, apply everything learned since 2020, and document the journey version by
version.

Sometimes the best way to appreciate how far you've come is to revisit where you started :-)

| Version | Goal | Fixes / Changes | Knowledge Gained |
| ------- | ---- | --------------- | ---------------- |
| v0.1.0 | Wrap signal handling in a channel-based API using `simple-signal` | Initial implementation with `mpsc` channel and `SignalReceiver` trait | `std::sync::mpsc`, traits, crate publishing basics |
| v0.2.0 | Symmetry and ergonomics | Added `SignalSender` trait, renamed methods, added convenience constructor | Trait design, API ergonomics, Rust naming conventions |
| v0.3.0 | Fix rough edges found by a fresh audit | Proper `Display` + `Error` impls, replaced panics with `Result`s, cleaner public API surface | Idiomatic error handling, `std::error::Error` trait chain |
| v0.4.0 | Correctness — the original design had a subtle but serious bug | Dropped `simple-signal`, rewrote with `libc`; self-pipe trick for async-signal-safe delivery; `Arc<Mutex<Vec<Sender>>>` fan-out for multiple subscribers | POSIX async-signal-safety, self-pipe pattern, FFI with `libc`, broadcast channel design |
| v0.5.0 | Safety and resilience | `// SAFETY:` docs on all `unsafe` blocks, `OsError(std::io::Error)` with `source()`, mutex poison recovery, named background thread, `fcntl` error checking, `try_listen()` | Unsafe code documentation, error chaining, `Mutex` poison semantics, non-blocking channel patterns |
| v0.6.0 | Compile-time API safety and final polish | Merged `prepare()` into `new()` (typestate pattern), all `unsafe` in private functions, `#![cfg(unix)]` platform gate, EINTR retry in dispatch loop, `Signal::from_raw` in `impl Signal`, corrected MSRV to 1.63 | Typestate pattern, POSIX EINTR semantics, platform gating, MSRV semantics |
| v0.7.0 | Signal semantics and iterator ergonomics | Added `SIGUSR1`, `SIGUSR2`, `SIGWINCH`, `SIGCONT`, `SIGURG`; `Signal::is_terminating()` to classify exit vs. informational signals; `Iterator` impl on `Receiver`; demo updated to loop using `for sig in receiver` | Signal categorization, `Iterator` for channel receivers, `loop`-as-expression pattern |

## Usage

```rust
use signal_msg::Signals;

fn main() {
    let signals = Signals::new().expect("failed to create signal handler");
    for sig in signals.subscribe() {
        println!("Got signal: {}", sig);
        if sig.is_terminating() { break; }
    }
}
```

## Examples

### Single Thread

Run the bundled demo in one terminal:

```bash
cargo run --example signal-msg-demo
```

then, in a second terminal, walk through all non-terminating signals before finishing with `SIGTERM`:

```bash
PID=$(pgrep -f signal-msg-demo)
for sig in USR1 USR2 WINCH CONT URG HUP PIPE ALRM; do
    kill -$sig $PID
    sleep 0.3
done
kill -0 $PID  # existence check only — not delivered to the process
kill -TERM $PID
```

which gives:

```bash
Got signal: SIGUSR1
Got signal: SIGUSR2
Got signal: SIGWINCH
Got signal: SIGCONT
Got signal: SIGURG
Got signal: SIGHUP
Got signal: SIGPIPE
Got signal: SIGALRM
Got signal: SIGTERM

Terminating on SIGTERM.
```

### Mutliple Threads

A second example demonstrates fan-out to multiple independent subscribers, each
running in its own thread:

```bash
cargo run --example signal-msg-multi
```

Send the same signal sequence from a second terminal (substituting
`signal-msg-multi` for the `pgrep` pattern):

```bash
PID=$(pgrep -f signal-msg-multi)
for sig in USR1 USR2 WINCH CONT URG HUP PIPE ALRM; do
    kill -$sig $PID
    sleep 0.3
done
kill -0 $PID  # existence check only — not delivered to the process
kill -TERM $PID
```

Each signal is delivered to both
subscribers independently; output ordering between them may vary:

```bash
[subscriber-1] Got signal: SIGUSR1
[subscriber-2] Got signal: SIGUSR1
[subscriber-1] Got signal: SIGWINCH
[subscriber-2] Got signal: SIGWINCH
...
[subscriber-1] Got signal: SIGTERM
[subscriber-2] Got signal: SIGTERM

Both subscribers exited.
```

## Credits

The project logo is derived from the "signpost" icon in the
[motorway](https://www.flaticon.com/packs/motorway-3) icon set by
[Freepik](https://www.flaticon.com/authors/freepik).

## License

Copyright © 2020-2026, Oxur Group

MIT License

[//]: ---Named-Links---

[logo]: assets/images/logo/v1-250x.png
[logo-large]: assets/images/logo/v1.png
[build]: https://github.com/oxur/signal-msg/actions?query=workflow%3Abuild+
[build-badge]: https://github.com/oxur/signal-msg/workflows/build/badge.svg
[crate]: https://crates.io/crates/signal-msg
[crate-badge]: https://img.shields.io/crates/v/signal-msg.svg
[docs]: https://docs.rs/signal-msg/
[docs-badge]: https://img.shields.io/badge/rust-documentation-blue.svg
[tag-badge]: https://img.shields.io/github/tag/oxur/signal-msg.svg
[tag]: https://github.com/oxur/signal-msg/tags
