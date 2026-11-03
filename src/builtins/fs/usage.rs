use crate::lang::command::OutputType::Known;
use crate::lang::data::table::ColumnFormat;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::Row;
use crate::lang::errors::CrushResult;
use crate::lang::pipe::TableOutputStream;
use crate::lang::signature::files;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::directory_lister::{DirectoryLister, directory_lister};
use signature::signature;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

static OUTPUT_TYPE: [ColumnType; 3] = [
    ColumnType::new_with_format("size", ColumnFormat::ByteUnit, ValueType::Integer, "the recursive size of the file or directory, in bytes."),
    ColumnType::new("blocks", ValueType::Integer, "the recursive number of blocks allocated to the file or directory."),
    ColumnType::new("file", ValueType::File, "the file or directory this row describes."),
];

#[signature(
    fs.usage,
    can_block = true,
    output = Known(ValueType::table_input_stream(& OUTPUT_TYPE)),
    short = "Calculate the recursive directory space usage.",
)]
pub struct Usage {
    #[unnamed()]
    #[description("the files to calculate the recursive size of.")]
    directory: Vec<Files>,

    #[description("do not show directory sizes for subdirectories.")]
    #[default(false)]
    silent: bool,

    #[description("write sizes for all files, not just directories.")]
    #[default(false)]
    all: bool,
}

fn size(
    path: &Path,
    silent: bool,
    all: bool,
    is_directory: bool,
    output: &TableOutputStream,
    lister: &impl DirectoryLister,
) -> CrushResult<(u64, u64)> {
    let mut sz = path.metadata().map(|m| m.size()).unwrap_or(0);
    let mut bl = path.metadata().map(|m| m.blocks()).unwrap_or(0);
    Ok(if is_directory {
        for child in lister.list(path)? {
            // `output` is already interruptible (`CommandContext::initialize_output`
            // wires its controller up), and `send` already checks it on every call --
            // but a child that isn't itself reported (a plain file, when `!all`) never
            // reaches a `send` at all, so this walk needs its own check between
            // children to actually notice `terminate`/`pause` on a tree with few or no
            // directories, same use case `TableOutputStream::poll_control`'s own doc
            // comment describes ("a command that waits for a long time between rows").
            output.poll_control()?;
            let (child_sz, child_bl) = size(
                &child.full_path,
                silent,
                all,
                child.is_directory,
                output,
                lister,
            )?;
            if (!silent && child.is_directory) || all {
                output.send(Row::new(vec![
                    Value::from(child_sz),
                    Value::from(child_bl),
                    Value::from(child.full_path),
                ]))?;
            }
            sz += child_sz;
            bl += child_bl;
        }
        (sz, bl)
    } else {
        (sz, bl)
    })
}

fn usage(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Usage = Usage::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.initialize_output(&OUTPUT_TYPE)?;

    let dirs = if !cfg.directory.is_empty() {
        files::into_paths(cfg.directory)?
    } else {
        vec![PathBuf::from(".")]
    };
    for file in dirs {
        let (sz, bl) = size(
            &file,
            cfg.silent,
            cfg.all,
            file.is_dir(),
            &output,
            &directory_lister(),
        )?;

        output.send(Row::new(vec![
            Value::Integer(sz as i128),
            Value::Integer(bl as i128),
            Value::from(file),
        ]))?
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::pipe::streams;
    use crate::util::directory_lister::Directory;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// A lister whose one directory yields `count` plain-file entries, each with a
    /// tiny real sleep before being produced. Simulates "many files, slow enough to
    /// observe mid-walk" without touching a real filesystem (keeping this test fast
    /// and deterministic) or adding a delay to the shared `FakeDirectoryLister` in
    /// directory_lister.rs, which other tests also use.
    struct SlowFakeLister {
        count: usize,
        visited: Arc<AtomicUsize>,
    }

    struct SlowFakeIter {
        remaining: usize,
        visited: Arc<AtomicUsize>,
    }

    impl Iterator for SlowFakeIter {
        type Item = Directory;

        fn next(&mut self) -> Option<Directory> {
            if self.remaining == 0 {
                return None;
            }
            self.remaining -= 1;
            self.visited.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_micros(200));
            Some(Directory {
                name: PathBuf::from(format!("f{}", self.remaining)),
                full_path: PathBuf::from(format!("/fake/f{}", self.remaining)),
                is_directory: false,
            })
        }
    }

    impl DirectoryLister for SlowFakeLister {
        type DirectoryIter = SlowFakeIter;

        fn list(&self, _path: impl Into<PathBuf>) -> CrushResult<Self::DirectoryIter> {
            Ok(SlowFakeIter {
                remaining: self.count,
                visited: self.visited.clone(),
            })
        }
    }

    #[test]
    fn size_is_interrupted_by_terminate() {
        let (output, _input) = streams(OUTPUT_TYPE.to_vec()).unwrap();
        let (output, controller) = output.interruptible();

        let visited = Arc::new(AtomicUsize::new(0));
        let lister = SlowFakeLister {
            count: 100_000,
            visited: visited.clone(),
        };

        let (done_tx, done_rx) = crossbeam::channel::bounded(1);
        std::thread::spawn(move || {
            let result = size(Path::new("/fake"), false, false, true, &output, &lister);
            let _ = done_tx.send(result.is_err());
        });

        // Give the spawned thread a moment to actually get into the loop.
        std::thread::sleep(Duration::from_millis(50));
        controller
            .terminate()
            .expect("controller's receiver was already dropped");

        match done_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(was_err) => assert!(
                was_err,
                "size() interrupted by terminate should return an error, not Ok"
            ),
            Err(_) => panic!("size() did not respect terminate within 5s -- it's still running"),
        }

        // The whole 100,000-entry walk, uninterrupted, would take ~20s (100,000 *
        // 200us) -- confirm it actually stopped early, not that it happened to
        // finish fast despite the delay.
        let visited_after = visited.load(Ordering::SeqCst);
        assert!(
            visited_after < 100_000,
            "expected size() to stop before visiting every entry, visited {} of 100000",
            visited_after
        );
    }
}
