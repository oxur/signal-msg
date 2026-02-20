use signal_msg::Signals;
use std::thread;

fn main() {
    let signals = Signals::new().expect("failed to create signal handler");

    // Each subscriber gets its own independent copy of every signal.
    let r1 = signals.subscribe();
    let r2 = signals.subscribe();

    let t1 = thread::spawn(move || {
        for sig in r1 {
            println!("[subscriber-1] Got signal: {}", sig);
            if sig.is_terminating() {
                break;
            }
        }
    });

    let t2 = thread::spawn(move || {
        for sig in r2 {
            println!("[subscriber-2] Got signal: {}", sig);
            if sig.is_terminating() {
                break;
            }
        }
    });

    t1.join().expect("subscriber-1 panicked");
    t2.join().expect("subscriber-2 panicked");

    println!("\nBoth subscribers exited.");
}
