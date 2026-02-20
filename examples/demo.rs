use signal_msg::Signals;

fn main() {
    let signals = Signals::new().expect("failed to create signal handler");
    let receiver = signals.subscribe();
    println!("Waiting for a signal...");
    match receiver.listen() {
        Ok(sig) => println!("Got signal: {}", sig),
        Err(e) => eprintln!("Error: {}", e),
    }
}
