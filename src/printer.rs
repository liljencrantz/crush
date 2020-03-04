use std::sync::mpsc::{channel, Sender};
use std::thread;
use crate::errors::{CrushError, CrushResult, to_crush_error};

enum PrinterMessage {
    Shutdown,
    JobError(CrushError),
    Error(Box<str>),
    Line(Box<str>),
    Lines(Vec<Box<str>>),
}

use crate::printer::PrinterMessage::*;
use crate::lang::JobJoinHandle;
use crate::thread_util::{handle, build};
use std::thread::JoinHandle;

#[derive(Clone)]
pub struct Printer {
    sender: Sender<PrinterMessage>,
}

impl Printer {

    pub fn new() -> (Printer, JoinHandle<()>) {
        let (sender, receiver) = channel();
        let handle = thread::Builder::new().name("printer".to_string()).spawn(move || {
            loop {
                match receiver.recv() {
                    Ok(message) => {
                        match message {
                            Shutdown => break,
                            Error(err) => println!("Error: {}", err),
                            JobError(err) => println!("Error: {}", err.message),
                            Line(line) => println!("{}", line),
                            Lines(lines) => for line in lines {println!("{}", line)},
                        }
                    }
                    Err(_) => break,
                }
            }
        }).unwrap();
        (Printer {
            sender,
        }, handle)
    }

    pub fn shutdown(self) {
        self.handle_error(to_crush_error(self.sender.send(PrinterMessage::Shutdown)));
    }

    pub fn line(&self, line: &str) {

        self.handle_error(to_crush_error(self.sender.send(PrinterMessage::Line(Box::from(line)))));
    }

    pub fn lines(&self, lines: Vec<Box<str>>) {
        self.handle_error(to_crush_error(self.sender.send(PrinterMessage::Lines(lines))));
    }

    pub fn handle_error<T>(&self, result: CrushResult<T>) {
        match result {
            Err(e) => self.job_error(e),
            _ => {}
        }
    }

    pub fn job_error(&self, err: CrushError) {
        self.sender.send(PrinterMessage::JobError(err));
    }

    pub fn error(&self, err: &str) {
        self.sender.send(PrinterMessage::Error(Box::from(err)));
    }

    pub fn join(&self, h: JobJoinHandle) {
        let local_printer = self.clone();
        handle(build("join".to_string()).spawn( move || {
            h.join(&local_printer);
            Ok(())
        }));
    }
}
