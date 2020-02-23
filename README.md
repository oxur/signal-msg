# signal-msg

[![][build-badge]][build]
[![][crate-badge]][crate]
[![][tag-badge]][tag]
[![][docs-badge]][docs]

[![][logo]][logo-large]

*Handle UNIX process signals with a shared channel (uses simple-signal)*

## About

This project aims to make simple signal handling even simpler: just use
messages. Since passing objects into an anonymous function signal handler can
get tricky, `signal-msg` offers an alternative approach of listening for
signals on a receiver.

## Usage

```rust
use std::sync::mpsc;
use signal_msg;
use signal_msg::SignalReceiver;

fn main() {
    let (signal_sender, signal_receiver) = mpsc::channel();
    signal_msg::handle(signal_sender);
    println!("Waiting for a signal...");
    let sig = signal_receiver.signal();
    println!("Got signal: {:?}", sig.unwrap());
}
```

## Credits

The project logo is derived from the "signpost" icon in the
[motorway](https://www.flaticon.com/packs/motorway-3) icon set by
[Freepik](https://www.flaticon.com/authors/freepik).


## License

Copyright © 2020, Oxur Group

MIT License

<!-- Named page links below: /-->

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