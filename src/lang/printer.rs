use crossbeam::Sender;
use crossbeam::Receiver;
use crossbeam::bounded;
use std::thread;
use crate::lang::errors::{CrushError, CrushResult, to_crush_error};

enum PrinterMessage {
    Shutdown,
    CrushError(CrushError),
    Error(Box<str>),
    Line(Box<str>),
//    Lines(Vec<Box<str>>),
}

use crate::lang::printer::PrinterMessage::*;
use std::thread::JoinHandle;
use lazy_static::lazy_static;

lazy_static! {
    static ref SND_RECV: StaticData = {
        let (sender, receiver) = bounded(128);
        StaticData { sender, receiver }
    };
}

#[derive(Clone)]
pub struct Printer {
    sender: Sender<PrinterMessage>,
}

struct StaticData {
    sender: Sender<PrinterMessage>,
    receiver: Receiver<PrinterMessage>,
}

pub fn printer() -> Printer {
    Printer {sender: SND_RECV.sender.clone()}
}

pub fn printer_thread() -> JoinHandle<()> {
    thread::Builder::new().name("printer".to_string()).spawn(move || {
        let mut open = true;
        while open || !SND_RECV.receiver.is_empty() {
            match SND_RECV.receiver.recv() {
                Ok(message) => {
                    match message {
                        Shutdown => open = false,
                        Error(err) => println!("Error: {}", err),
                        CrushError(err) => println!("Error: {}", err.message),
                        Line(line) => println!("{}", line),
//                        Lines(lines) => for line in lines {println!("{}", line)},
                    }
                }
                Err(_) => break,
            }
        }
    }).unwrap()
}

impl Printer {

    pub fn shutdown(self) {
        self.handle_error(to_crush_error(self.sender.send(PrinterMessage::Shutdown)));
    }

    pub fn line(&self, line: &str) {
        self.handle_error(to_crush_error(self.sender.send(PrinterMessage::Line(Box::from(line)))));
    }
/*
    pub fn lines(&self, lines: Vec<Box<str>>) {
        self.handle_error(to_crush_error(self.sender.send(PrinterMessage::Lines(lines))));
    }
*/
    pub fn handle_error<T>(&self, result: CrushResult<T>) {
        match result {
            Err(e) => self.crush_error(e),
            _ => {}
        }
    }

    pub fn crush_error(&self, err: CrushError) {
        let _ = self.sender.send(PrinterMessage::CrushError(err));
    }

    pub fn error(&self, err: &str) {
        let _ = self.sender.send(PrinterMessage::Error(Box::from(err)));
    }
}
