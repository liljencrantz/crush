use std::fs;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use test_finder::test_finder;
use ctor::{ctor, dtor};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

fn run_system_test(name: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_crush"))
        .args(&[name.to_str().unwrap()])
        .output()
        .expect("failed to execute process");

    let status_name = name.with_extension("crush.status");
    let expected_status: i32 = match fs::read_to_string(&status_name) {
        Ok(s) => s.trim().parse::<i32>().unwrap_or_else(|_| {
            panic!(
                "failed to parse expected exit status from {}: {:?}",
                status_name.to_str().unwrap(),
                s
            )
        }),
        Err(_) => 0,
    };
    assert_eq!(
        output.status.code(),
        Some(expected_status),
        "Wrong exit status while running file {}. Expected {}, got {:?}.\nStdout:\n{}\nStderr:\n{}",
        name.to_str().unwrap(),
        expected_status,
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // A test with no .crush.output file skips the output comparison entirely -- it's
    // only checking the exit status (via a .crush.status file, or the default-0 check
    // above).
    let output_name = name.with_extension("crush.output");
    let expected_output = match fs::read_to_string(&output_name) {
        Ok(s) => s,
        Err(_) => return,
    };
    let expected_lines = expected_output.lines().collect::<Vec<&str>>();
    let actual_string = String::from_utf8_lossy(&output.stdout);
    let actual_lines = actual_string.lines().collect::<Vec<&str>>();

    for (idx, (expected, actual)) in expected_lines.iter().zip(actual_lines.iter()).enumerate() {
        assert_eq!(
            actual,
            expected,
            "Error on line {} of output while running file {}.",
            idx+1,
            name.to_str().unwrap()
        );
    }

    // The loop above only compares the overlapping prefix, via zip(), so it can't by
    // itself catch a run that produces a different number of lines than expected (e.g.
    // one that now errors out partway through and produces fewer lines, or that emits
    // unexpected trailing output). Check the lengths too.
    assert_eq!(
        actual_lines.len(),
        expected_lines.len(),
        "Wrong number of output lines while running file {}. Expected {} lines, got {}.\n\
         Expected output:\n{}\nActual output:\n{}",
        name.to_str().unwrap(),
        expected_lines.len(),
        actual_lines.len(),
        expected_output,
        actual_string,
    );
}

// dns-service/grpc-service/ssh-service are started once, here, before any test runs --
// not per-test the way they used to be -- so that tests/dns_query.crush,
// tests/grpc_mirror.crush, and tests/ssh_exec*.crush can be plain, auto-discovered
// tests/*.crush golden files with no custom Rust wiring of their own, exactly like
// every other system test. `#[ctor]`/`#[dtor]` (not a plain #[test] fn) are the only
// hook Rust's test harness offers for "run once before/after every test in this
// binary" -- test_finder!()-generated tests have no body of their own to start a
// server from, and cargo test's own generated `main()` isn't something this crate can
// edit. All three servers use fixed ports and (for ssh-service) a fixed, committed host
// key specifically so the test scripts that talk to them never need a value injected at
// run time; see each service's own src/main.rs for why that's safe here.
static TEST_SERVERS: OnceLock<Mutex<Vec<Child>>> = OnceLock::new();

fn wait_for_port(addr: &str) {
    let start = std::time::Instant::now();
    let max_wait = Duration::from_secs(60);
    let mut backoff = Duration::from_millis(1);
    loop {
        if std::net::TcpStream::connect(addr).is_ok() {
            return;
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < max_wait,
            "test server never started listening on {} within 60s",
            addr
        );
        std::thread::sleep(backoff.min(max_wait - elapsed));
        backoff = (backoff * 2).min(max_wait);
    }
}

fn spawn(path: &str, extra_args: &[&str]) -> Child {
    Command::new(path)
        .args(extra_args)
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start {}: {}", path, e))
}

// dns-service/grpc-service/ssh-service are workspace members with their own [[bin]]
// targets, not dependencies of this crate -- so unlike `crush` itself, Cargo has no
// CARGO_BIN_EXE_<name> env var for them (that mechanism only covers binaries of the
// *current* package), and neither `cargo test --workspace` nor `cargo llvm-cov
// --workspace` builds them on their own: both restrict target selection to test
// harnesses (`cargo test`'s underlying invocation passes `--tests`), so a package with
// only a plain `[[bin]]` and no tests of its own is never built as a side effect.
//
// So this builds them itself, once, on demand, with an explicit `cargo build -p ...`
// targeting the exact directory `CARGO_BIN_EXE_crush` was itself built into -- plain
// target/debug for `cargo test`, target/llvm-cov-target/debug for `cargo llvm-cov
// --workspace`. Because this runs as a genuine child process of that same (possibly
// RUSTC_WRAPPER-wrapped) cargo invocation, it inherits the wrapper/coverage env vars
// automatically, so the resulting binaries end up instrumented exactly like `crush`
// itself under `cargo llvm-cov` -- confirmed by checking for __llvm_profile symbols in
// the resulting binaries. (An earlier attempt at this via the `escargot` crate appeared
// to produce uninstrumented binaries no matter how it was pointed; that was actually a
// stale-fingerprint artifact from these two packages not having been rebuilt in days --
// Cargo's fingerprint doesn't treat RUSTC_WRAPPER as a cache-invalidating input, so it
// silently relinked old unwrapped object files. `cargo clean -p dns-service -p
// ssh-service` followed by a fresh build resolved it; a real dependency on the crate
// wasn't the fix and was reverted.)
fn build_siblings_and_locate(name: &str) -> std::path::PathBuf {
    let crush_bin = std::path::Path::new(env!("CARGO_BIN_EXE_crush"));
    // .../<target-dir>/debug/crush -> .../<target-dir>
    let target_dir = crush_bin
        .parent()
        .and_then(Path::parent)
        .expect("CARGO_BIN_EXE_crush should be two levels under the target dir");

    let status = Command::new("cargo")
        .args(["build", "-p", "dns-service", "-p", "grpc-service", "-p", "ssh-service"])
        .arg("--target-dir")
        .arg(target_dir)
        .status()
        .expect("failed to invoke cargo to build dns-service/grpc-service/ssh-service");
    assert!(
        status.success(),
        "cargo build -p dns-service -p grpc-service -p ssh-service failed"
    );

    crush_bin.with_file_name(name)
}

#[ctor]
fn start_test_servers() {
    let mut children = Vec::new();

    // A single `cargo build -p ...` call builds all three siblings; only the first
    // call's path is used directly here, the other two are cheap same-directory lookups
    // once the build above has already happened.
    let dns_service = build_siblings_and_locate("dns-service");
    let target_dir = dns_service
        .parent()
        .expect("dns-service path should have a parent directory")
        .to_path_buf();
    let grpc_service = target_dir.join("grpc-service");
    let ssh_service = target_dir.join("ssh-service");

    children.push(spawn(dns_service.to_str().unwrap(), &[]));
    wait_for_port("127.0.0.1:20053");

    children.push(spawn(grpc_service.to_str().unwrap(), &[]));
    wait_for_port("[::1]:50051");

    // ssh-service defaults to spawning "./target/debug/crush" for each exec channel's
    // `crush --pup` if not told otherwise -- the same stale-path problem
    // CARGO_BIN_EXE_crush fixes here, just one process further out. Pass the real one
    // explicitly, so the pup wire round trip these tests are meant to exercise reaches
    // the actual instrumented crush binary rather than whatever debug build happens to
    // already exist on disk.
    children.push(spawn(
        ssh_service.to_str().unwrap(),
        &[env!("CARGO_BIN_EXE_crush")],
    ));
    wait_for_port("127.0.0.1:2849");

    TEST_SERVERS
        .set(Mutex::new(children))
        .unwrap_or_else(|_| panic!("start_test_servers ran more than once"));
}

// A plain SIGKILL (std::process::Child::kill's only option on Unix) gives coverage
// instrumentation's atexit-based profraw flush no chance to run, so an
// instrumented-but-SIGKILLed service binary always reports 0% coverage regardless of
// what its own tests actually exercised -- confirmed by comparing report output before
// and after switching this to SIGTERM. SIGTERM lets each service's normal signal
// handling (or, absent one, the default terminate-after-atexit-hooks-run behavior) exit
// it cleanly; a short grace period with a SIGKILL fallback keeps this from hanging if a
// service doesn't respond to SIGTERM.
#[dtor]
fn stop_test_servers() {
    if let Some(m) = TEST_SERVERS.get() {
        let mut children = m.lock().unwrap();
        for child in children.iter_mut() {
            let _ = kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM);
        }
        for child in children.iter_mut() {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
    }
}

test_finder!();
