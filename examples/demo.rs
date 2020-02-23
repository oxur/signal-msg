use signal_msg;
use signal_msg::{SignalReceiver, SignalSender};
use std::sync::mpsc;

fn main() {
    let (signal_sender, signal_receiver) = mpsc::channel();
    signal_sender.prepare_signals();
    println!("Waiting for a signal...");
    let sig = signal_receiver.listen();
    println!("Got signal: {:?}", sig.unwrap());
}
