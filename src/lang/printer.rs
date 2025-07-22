/**
    Crush uses a single thread to perform all output printing. This prevents torn lines and other
    visual problems. Output is sent to the print thread via a Printer.
*/
use crate::lang::errors::{CrushError, CrushErrorType, CrushResult};
use crate::lang::printer::PrinterMessage::*;
use crossbeam::channel::Receiver;
use crossbeam::channel::Sender;
use crossbeam::channel::bounded;
use std::cmp::max;
use std::collections::HashMap;
use std::thread;
use std::thread::JoinHandle;
use termion::terminal_size;
use crate::lang::state::scope::Scope;
use crate::util::highlight::highlight_colors;
use crate::util::md::render;

pub enum PrinterMessage {
    Ping,
    CrushError(CrushError),
    Error(String),
    Line(String),
}

/**
    The thing you use to send messages to the print thread.

    It is relatively small, and can be cloned when convenient.
*/
#[derive(Clone)]
pub struct Printer {
    sender: Sender<PrinterMessage>,
    pong_receiver: Receiver<()>,
}

// Too small terminals mean we can't meaningfully print anything, so assume at least this size
const TERMINAL_MIN_WIDTH: usize = 10;
const TERMINAL_MIN_HEIGHT: usize = 5;

// Terminal size to assume if terminal_size() call fails
const TERMINAL_FALLBACK_WIDTH: usize = 80;
const TERMINAL_FALLBACK_HEIGHT: usize = 30;

/**
    Create a print thread and a printer that sends messages to it.
    Create additional printer instances by cloning it. To exit the print thread,
    simply drop all Printer instances connected to it, and the thread will exit.
    To wait until the print thread has exited, call join on the JoinHandle.
*/
pub fn init(scope: Option<Scope>) -> (Printer, JoinHandle<()>) {
    let (sender, receiver) = bounded(128);
    let (pong_sender, pong_receiver) = bounded(1);

    (
        Printer {
            sender,
            pong_receiver,
        },
        thread::Builder::new()
            .name("printer".to_string())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    match message {
                        Ping => {
                            let _ = pong_sender.send(());
                        }
                        Error(err) => eprintln!("{}", err),
                        CrushError(err) => {
                            let colors = scope.as_ref().map(|s| highlight_colors(s)).unwrap_or_else(|| HashMap::new());
                            let message = match err.command() {
                                Some(cmd) if !err.message().starts_with('`')=> {
                                    format!("`{}`: {}", cmd, err.message())
                                },
                                _ => err.message(),
                            };
                            let rendered = render(&message, 80, colors).unwrap_or_else(|_| err.message());
                            eprintln!("{}", rendered);
                            if let Some(ctx) = err.source() {
                                match ctx.show() {
                                    Ok(ctx) => eprintln!("{}", ctx),
                                    Err(_) => {}
                                }
                            }

                            if let Some(trace) = err.trace() {
                                eprintln!("Stack trace:");
                                eprintln!("{}", trace);
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

/**
   Create a Printer instance that doesn't actually print. A print thread actually still exists,
   but it does not print anything.

   It would be more performant to rewrite this to not even spawn a thread and just drop whatever
   you pass in, but Crush currently is not optimized for speed.
*/
pub fn noop() -> (Printer, JoinHandle<()>) {
    let (sender, receiver) = bounded(128);
    let (pong_sender, pong_receiver) = bounded(1);

    (
        Printer {
            sender,
            pong_receiver,
        },
        thread::Builder::new()
            .name("printer:noop".to_string())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    match message {
                        Ping => {
                            let _ = pong_sender.send(());
                        }
                        _ => {}
                    }
                }
            })
            .unwrap(),
    )
}

impl Printer {
    /**
        Send one line of preformated output to the printer.
    */
    pub fn line(&self, line: &str) {
        self.handle_error(
            self.sender
                .send(Line(line.to_string()))
                .map_err(|e| e.into()),
        );
    }

    /**
        If the passed in result is an error, print information about it.
    */
    pub fn handle_error<T>(&self, result: CrushResult<T>) {
        if let Err(e) = result {
            self.crush_error(e);
        }
    }

    /**
    Send a ping to the printer and await a reply. Because messages from a given thread are performed
    in order, this ensures that all output passed on to the printer from this thread have been
    printed. Sending a ping before displaying a prompt lower the risk of stray output while showing
    the prompt. Note that messages from other threads aren't necessarily processed in order, so
    this isn't a foolproof method to fully avoid stray output.
    */
    pub fn ping(&self) {
        if let Ok(_) = self.sender.send(PrinterMessage::Ping) {
            let _ = self.pong_receiver.recv();
        }
    }
    
    /**
       Print information about the passed in error.
    */
    pub fn crush_error(&self, err: CrushError) {
        match &err.error_type() {
            CrushErrorType::SendError(_) => {}
            _ => {
                _ = self
                    .sender
                    .send(PrinterMessage::CrushError(err));
            }
        }
    }

    /**
       Print the passed in, pre-formated error.
    */
    pub fn error(&self, err: &str) {
        let _ = self
            .sender
            .send(PrinterMessage::Error(format!("Error: {}", err.to_string())));
    }

    /**
     The width (in characters) of the console we're printing to.
    */
    pub fn width(&self) -> usize {
        match terminal_size() {
            Ok(s) => max(TERMINAL_MIN_WIDTH, s.0 as usize),
            Err(_) => TERMINAL_FALLBACK_WIDTH,
        }
    }

    /**
    The height (in characters) of the console we're printing to.
     */
    pub fn height(&self) -> usize {
        match terminal_size() {
            Ok(s) => max(TERMINAL_MIN_HEIGHT, s.1 as usize),
            Err(_) => TERMINAL_FALLBACK_HEIGHT,
        }
    }
}
