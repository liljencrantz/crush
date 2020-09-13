use crate::lang::errors::{to_crush_error, CrushError, CrushResult, CrushErrorType};
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
use std::cmp::max;
use crate::lang::ast::Location;

#[derive(Clone)]
pub struct Printer {
    source: Option<(String, Location)>,
    sender: Sender<PrinterMessage>,
}

pub fn init() -> (Printer, JoinHandle<()>) {
    let (sender, receiver) = bounded(128);

    (
        Printer {
            sender: sender,
            source: None,
        },
        thread::Builder::new()
            .name("printer".to_string())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    match message {
                        Error(err) => eprintln!("Error: {}", err),
                        CrushError(err) => {
                            eprintln!("Error: {}", err.message());
                            if let Some(ctx) = err.context() {
                                eprintln!("{}", ctx);
                            }
                        }
                        Line(line) => println!("{}", line),
                        //                        Lines(lines) => for line in lines {println!("{}", line)},
                    }
                }
            })
            .unwrap(),
    )
}

pub fn noop() -> (Printer, JoinHandle<()>) {
    let (sender, receiver) = bounded(128);

    (
        Printer {
            sender: sender,
            source: None,
        },
        thread::Builder::new()
            .name("printer:noop".to_string())
            .spawn(move || {
                while let Ok(_) = receiver.recv() {}
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
            if !e.is(CrushErrorType::SendError) {
                self.crush_error(e)
            }
        }
    }

    pub fn with_source(&self, def: &str, location: Location) -> Printer {
        Printer {
            sender: self.sender.clone(),
            source: Some((def.to_string(), location)),
        }
    }

    pub fn crush_error(&self, err: CrushError) {
        let _ = self.sender.send(PrinterMessage::CrushError(err.with_source(&self.source)));
    }

    pub fn error(&self, err: &str) {
        let _ = self.sender.send(PrinterMessage::Error(err.to_string()));
    }

    pub fn width(&self) -> usize {
        match terminal_size() {
            Ok(s) => max(10, s.0 as usize),
            Err(_) => 80,
        }
    }

    pub fn height(&self) -> usize {
        match terminal_size() {
            Ok(s) => max(s.1 as usize, 5),
            Err(_) => 30,
        }
    }
}
