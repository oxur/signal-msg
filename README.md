# signal-msg

[![][build-badge]][build]
[![][crate-badge]][crate]
[![][tag-badge]][tag]
[![][docs-badge]][docs]

*Handle UNIX process signals with a shared channel (uses simple-signal)*

[![][logo]][logo-large]

## About

This project aims to make simple signal handling even simpler: just use
messages. Since passing objects into an anonymous function signal handler can
get tricky, `signal-msg` offers an alternative approach of listening for
signals on a receiver.

This library was created for ease of use when setting up examples that needed a
quick and easy setup for signal handling, providing a message-based solution
around the [simple-signal](https://github.com/swizard0/rust-simple-signal)
library. A more robust (if also more verbose) solution is possible when using
the [signal-hook](https://github.com/vorner/signal-hook) library.

Similar functionality to signal-msg is provided by the
[signal-notify](https://crates.io/crates/signal-notify) and
[chan-signal](https://crates.io/crates/chan-signal) libraries (note, though,
that the latter is deprecated).)

## Usage

```rust
use signal_msg::Signals;

fn main() {
    let signals = Signals::new().expect("failed to create signal handler");
    signals.prepare();
    let receiver = signals.subscribe();
    println!("Waiting for a signal...");
    match receiver.listen() {
        Ok(sig) => println!("Got signal: {}", sig),
        Err(e)  => eprintln!("Error: {}", e),
    }
}
```

## Example

Run the bundled demo, then send it a signal (e.g. Ctrl-C):

```bash
cargo run --example demo
```

## Credits

The project logo is derived from the "signpost" icon in the
[motorway](https://www.flaticon.com/packs/motorway-3) icon set by
[Freepik](https://www.flaticon.com/authors/freepik).

## License

Copyright © 2020-2026, Oxur Group

MIT License

[//]: ---Named-Links---

[logo]: resources/images/logo-250x.png
[logo-large]: resources/images/logo-1000x.png
[build]: https://github.com/oxur/signal-msg/actions?query=workflow%3Abuild+
[build-badge]: https://github.com/oxur/signal-msg/workflows/build/badge.svg
[crate]: https://crates.io/crates/signal-msg
[crate-badge]: https://img.shields.io/crates/v/signal-msg.svg
[docs]: https://docs.rs/signal-msg/
[docs-badge]: https://img.shields.io/badge/rust-documentation-blue.svg
[tag-badge]: https://img.shields.io/github/tag/oxur/signal-msg.svg
[tag]: https://github.com/oxur/signal-msg/tags
