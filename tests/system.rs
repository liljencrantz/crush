use std::fs;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use test_finder::test_finder;
use assert_cmd::prelude::*;
use ctor::{ctor, dtor};

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

fn build_and_spawn(package: &str, extra_args: &[&str]) -> Child {
    let run = escargot::CargoBuild::new()
        .bin(package)
        .package(package)
        .run()
        .unwrap_or_else(|e| panic!("Failed to build {} binary: {}", package, e));
    Command::new(run.path())
        .args(extra_args)
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start {}: {}", package, e))
}

#[ctor]
fn start_test_servers() {
    let mut children = Vec::new();

    children.push(build_and_spawn("dns-service", &[]));
    wait_for_port("127.0.0.1:20053");

    children.push(build_and_spawn("grpc-service", &[]));
    wait_for_port("[::1]:50051");

    // ssh-service defaults to spawning "./target/debug/crush" for each exec channel's
    // `crush --pup` if not told otherwise -- the same stale-path problem
    // CARGO_BIN_EXE_crush fixed for this test binary itself, just one process further
    // out. Left as the default, this would silently run whatever plain debug binary
    // happens to already exist under `cargo llvm-cov test` (a separate, uninstrumented
    // build lives there too), so the pup wire round trip these tests are meant to
    // exercise would never show up in coverage. Pass the real one explicitly.
    children.push(build_and_spawn(
        "ssh-service",
        &[env!("CARGO_BIN_EXE_crush")],
    ));
    wait_for_port("127.0.0.1:2849");

    TEST_SERVERS
        .set(Mutex::new(children))
        .unwrap_or_else(|_| panic!("start_test_servers ran more than once"));
}

#[dtor]
fn stop_test_servers() {
    if let Some(m) = TEST_SERVERS.get() {
        let mut children = m.lock().unwrap();
        for child in children.iter_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn test_remote_host_file() {
    // remote:host:list/remote:host:remove (src/builtins/remote.rs) never connect
    // anywhere -- they just read/rewrite a known_hosts file -- so a static fixture is
    // enough; unlike the ssh-service-backed tests, no live server is involved. Three
    // throwaway public keys, generated once with `ssh-keygen -t ed25519`/`-t rsa` and
    // never used to authenticate anywhere; public keys carry no secret material.
    const FIXTURE: &str = "\
host-a.example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHFWjNefhOjX7XiOQ7/66ALKB6ru8AaaMJCxQlpp9KuQ
host-b.example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDrlzEyFYijCFoXNf3SLmwDUT8DQwxEVtdb6dQ/ajL89
host-c.example.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQDRSEVxK2sm2QXPbIzufX9EvwiQWZeduizmKDayFO2d02L7H5iKbcO8TjKz5zCoNfcdp1weEGdm0YQ+L85vTGjpv3CYbA8YtbLoFXslxi4TcCWBZ1qZHCeDsSY4jWmdXQMRlOwrtK19qLoCnDgqvtLQMU+/YWZpAWFVm3crWiRLgFCIet+MpDggmujlWMAeZ1HnmcI7suGQeYx7ufiuKsWsST4ks+O/n3dKzpi4WEK7Z/Bd3ZA8UpBWvmiezllKooA82QqzEb1VYCTkmnaHgaIsigHnQKVTUtiN196ot3n0waHGNrGYmka8/ukbtv9emAD2A5+pceWEt2/s36x9FrCr
";

    // A fresh copy every run -- host:remove mutates the file, so this must never point
    // at the fixture text's own (nonexistent) source location.
    let path = std::env::temp_dir().join("crush_test_host_list_remove_known_hosts");
    fs::write(&path, FIXTURE).expect("failed to write known_hosts fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_crush"))
        .args(&["tests/remote/host_list_remove.crush"])
        .env("CRUSH_TEST_HOST_FIXTURE", &path)
        .output()
        .expect("failed to execute process");

    assert_eq!(
        output.status.code(),
        Some(0),
        "host_list_remove.crush failed.\nStdout:\n{}\nStderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

test_finder!();
