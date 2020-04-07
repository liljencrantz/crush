use crossbeam::Sender;
use crossbeam::bounded;
use std::thread;
use crate::lang::errors::{CrushError, CrushResult, to_crush_error, Kind};

enum PrinterMessage {
    CrushError(CrushError),
    Error(Box<str>),
    Line(Box<str>),
//    Lines(Vec<Box<str>>),
}

use crate::lang::printer::PrinterMessage::*;
use std::thread::JoinHandle;

#[derive(Clone)]
pub struct Printer {
    sender: Sender<PrinterMessage>,
}

pub fn init() -> (Printer, JoinHandle<()>) {
    let (sender, receiver) = bounded(128);

    (Printer { sender: sender.clone() },
    thread::Builder::new().name("printer".to_string()).spawn(move || {
        loop {
            match receiver.recv() {
                Ok(message) => {
                    match message {
                        Error(err) => println!("Error: {}", err),
                        CrushError(err) => println!("Error: {}", err.message),
                        Line(line) => println!("{}", line),
//                        Lines(lines) => for line in lines {println!("{}", line)},
                    }
                }
                Err(_) => break,
            }
        }
    }).unwrap())

}

impl Printer {

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
            Err(e) => {
                match e.kind {
                    Kind::SendError => {}
                    _ => self.crush_error(e),
                }
            }
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
