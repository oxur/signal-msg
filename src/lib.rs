use simple_signal::{self};
use std::sync::mpsc;

// Copied from https://github.com/swizard0/rust-simple-signal/blob/master/src/lib.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Signal {
    Hup,
    Int,
    Quit,
    Ill,
    Abrt,
    Fpe,
    Kill,
    Segv,
    Pipe,
    Alrm,
    Term,
}

pub fn all() -> Vec<simple_signal::Signal> {
    vec![
        simple_signal::Signal::Hup,  // 0
        simple_signal::Signal::Int,  // 1
        simple_signal::Signal::Ill,  // 3
        simple_signal::Signal::Abrt, // 4
        simple_signal::Signal::Fpe,  // 5
        simple_signal::Signal::Pipe, // 8
        simple_signal::Signal::Alrm, // 9
        simple_signal::Signal::Term, // 10
    ]
}

pub fn from_i32(sig_num: i32) -> Result<Signal, String> {
    match sig_num {
        0 => Ok(Signal::Hup),
        1 => Ok(Signal::Int),
        3 => Ok(Signal::Ill),
        4 => Ok(Signal::Abrt),
        5 => Ok(Signal::Fpe),
        8 => Ok(Signal::Pipe),
        9 => Ok(Signal::Alrm),
        10 => Ok(Signal::Term),
        _ => Err(format!("Got unsupported signal: {:?}", sig_num)),
    }
}

pub trait SignalSender {
    fn prepare_signals(&self);
}

impl SignalSender for mpsc::Sender<i32> {
    fn prepare_signals(&self) {
        let s = self.clone();
        simple_signal::set_handler(&all(), move |signals| {
            for sig in signals {
                s.send(*sig as i32).unwrap();
            }
        });
    }
}

pub trait SignalReceiver {
    fn listen(&self) -> Result<Signal, String>;
}

impl SignalReceiver for mpsc::Receiver<i32> {
    fn listen(&self) -> Result<Signal, String> {
        from_i32(self.recv().unwrap())
    }
}

pub fn new() -> (mpsc::Sender<i32>, mpsc::Receiver<i32>) {
    mpsc::channel()
}
