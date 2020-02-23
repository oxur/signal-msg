
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