use chrono::{Duration, Local};
use crossbeam::channel::Receiver;
use crate::lang::errors::{terminate, CrushResult};
use crate::lang::job_control::StreamControlMessage;

pub fn interruptible_sleep(duration: &Duration, control: &Receiver<StreamControlMessage>) -> CrushResult<()> {
    let mut time_left = duration.clone();
    loop {
        let start_sleep = Local::now();
        match control.recv_timeout(time_left.to_std()?) {
            Ok(StreamControlMessage::Terminate) => return terminate(),
            Ok(StreamControlMessage::Pause) => {
                let sleep_duration = Local::now() - start_sleep;
                if sleep_duration > time_left {
                    return Ok(());
                }
                loop {
                    match control.recv() {
                        Ok(StreamControlMessage::Terminate) => return terminate(),
                        Ok(StreamControlMessage::Resume) => {
                            time_left -= sleep_duration;
                            break;
                        },
                        Ok(StreamControlMessage::Pause) => {}
                        Err(_) => return terminate(),
                    }
                }
            },
            // A genuine timeout (Err) means the full duration elapsed with no
            // interruption at all -- the sleep is done. A stray Resume received
            // while not paused isn't really meaningful, but is treated the same
            // way the original single-shot sleep implementations did: as "done",
            // not as a reason to keep looping with the same time_left forever.
            Ok(StreamControlMessage::Resume) | Err(_) => return Ok(()),
        }
    }
}
