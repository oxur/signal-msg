use signal_msg::{self, SignalReceiver, SignalSender};

fn main() {
    let (signal_sender, signal_receiver) = signal_msg::new();
    signal_sender.prepare_signals();
    println!("Waiting for a signal...");
    let sig = signal_receiver.listen();
    println!("Got signal: {:?}", sig.unwrap());
}
