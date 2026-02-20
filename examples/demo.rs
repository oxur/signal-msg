use signal_msg::{self, SignalReceiver, SignalSender};

fn main() {
    let (signal_sender, signal_receiver) = signal_msg::new();
    signal_sender.prepare_signals();
    println!("Waiting for a signal...");
    match signal_receiver.listen() {
        Ok(sig) => println!("Got signal: {}", sig),
        Err(e)  => eprintln!("Error: {}", e),
    }
}
