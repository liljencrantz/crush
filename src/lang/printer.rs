use crate::lang::errors::{to_crush_error, CrushError, CrushResult};
use crossbeam::bounded;
use crossbeam::Sender;
use std::thread;

enum PrinterMessage {
    CrushError(CrushError),
    Error(String),
    Line(String),
    //    Lines(Vec<String>),
}

use crate::lang::printer::PrinterMessage::*;
use std::thread::JoinHandle;
use termion::terminal_size;

#[derive(Clone)]
pub struct Printer {
    sender: Sender<PrinterMessage>,
}

pub fn init() -> (Printer, JoinHandle<()>) {
    let (sender, receiver) = bounded(128);

    (
        Printer { sender: sender },
        thread::Builder::new()
            .name("printer".to_string())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    match message {
                        Error(err) => eprintln!("Error: {}", err),
                        CrushError(err) => eprintln!("Error: {}", err.message()),
                        Line(line) => println!("{}", line),
                        //                        Lines(lines) => for line in lines {println!("{}", line)},
                    }
                }
            })
            .unwrap(),
    )
}

impl Printer {
    pub fn line(&self, line: &str) {
        self.handle_error(to_crush_error(
            self.sender.send(PrinterMessage::Line(line.to_string())),
        ));
    }
    /*
        pub fn lines(&self, lines: Vec<String>) {
            self.handle_error(to_crush_error(self.sender.send(PrinterMessage::Lines(lines))));
        }
    */
    pub fn handle_error<T>(&self, result: CrushResult<T>) {
        if let Err(e) = result {
            if e != CrushError::SendError {
                self.crush_error(e)
            }
        }
    }

    pub fn crush_error(&self, err: CrushError) {
        let _ = self.sender.send(PrinterMessage::CrushError(err));
    }

    pub fn error(&self, err: &str) {
        let _ = self.sender.send(PrinterMessage::Error(err.to_string()));
    }

    pub fn width(&self) -> usize {
        match terminal_size() {
            Ok(s) => s.0 as usize,
            Err(_) => 80,
        }
    }

    pub fn height(&self) -> usize {
        match terminal_size() {
            Ok(s) => s.1 as usize,
            Err(_) => 30,
        }
    }
}
