use signal_msg::Signals;

fn main() {
    let signals = Signals::new().expect("failed to create signal handler");
    println!("Waiting for signals...");
    println!("(try SIGUSR1, SIGWINCH, SIGCONT; send SIGINT or SIGTERM to exit)\n");

    for sig in signals.subscribe() {
        println!("Got signal: {}", sig);
        if sig.is_terminating() {
            println!("\nTerminating on {}.", sig);
            break;
        }
    }
}
