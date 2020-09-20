use crate::lang::errors::{to_crush_error, CrushError, CrushResult, CrushErrorType};
use crossbeam::bounded;
use crossbeam::Sender;
use crossbeam::Receiver;
use std::thread;

enum PrinterMessage {
    Ping,
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
    pong_receiver: Receiver<()>,
}

// Too small terminals mean we can't meaningfully print anything, so assume at least this size
const TERMINAL_MIN_WIDTH: usize = 10;
const TERMINAL_MIN_HEIGHT: usize = 5;

// Terminal size to assume if terminal_size() call fails
const TERMINAL_FALLBACK_WIDTH: usize = 80;
const TERMINAL_FALLBACK_HEIGHT: usize = 30;

pub fn init() -> (Printer, JoinHandle<()>) {
    let (sender, receiver) = bounded(128);
    let (pong_sender, pong_receiver) = bounded(1);

    (
        Printer {
            sender,
            pong_receiver,
            source: None,
        },
        thread::Builder::new()
            .name("printer".to_string())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    match message {
                        Ping => { pong_sender.send(()); }
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
    let (pong_sender, pong_receiver) = bounded(1);

    (
        Printer {
            sender,
            source: None,
            pong_receiver,
        },
        thread::Builder::new()
            .name("printer:noop".to_string())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    match message {
                        Ping => { pong_sender.send(()); }
                        _ => {}
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
            if !e.is(CrushErrorType::SendError) {
                self.crush_error(e)
            }
        }
    }

    pub fn ping(&self) {
        if let Ok(_) = self.sender.send(PrinterMessage::Ping) {
            let _ = self.pong_receiver.recv();
        }
    }

    pub fn with_source(&self, def: &str, location: Location) -> Printer {
        Printer {
            sender: self.sender.clone(),
            source: Some((def.to_string(), location)),
            pong_receiver: self.pong_receiver.clone(),
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
            Ok(s) => max(TERMINAL_MIN_WIDTH, s.0 as usize),
            Err(_) => TERMINAL_FALLBACK_WIDTH,
        }
    }

    pub fn height(&self) -> usize {
        match terminal_size() {
            Ok(s) => max(TERMINAL_MIN_HEIGHT, s.1 as usize),
            Err(_) => TERMINAL_FALLBACK_HEIGHT,
        }
    }
}
