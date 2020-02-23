
use std::sync::mpsc;
use signal_msg;

fn main() {
    let (signal_sender, signal_receiver) = mpsc::channel();
    signal_msg::handle(signal_sender);
    println!("Waiting for a signal...");
    let sig_num = signal_receiver.recv().unwrap();
    let sig = signal_msg::from_i32(sig_num);
    println!("Got signal: {:?}", sig.unwrap());
}
