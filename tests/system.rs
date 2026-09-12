use std::{fs, thread};
use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use test_finder::test_finder;
use assert_cmd::prelude::*;

fn run_system_test(name: &Path) {
    let output = Command::new("./target/debug/crush")
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

#[test]
fn test_grpc() {
    let run = escargot::CargoBuild::new()
        .bin("grpc-service")
        .package("grpc-service") // Name of the sub-crate
        .run()
        .expect("Failed to build grpc-service binary");

    let mut server = Command::new(run.path())
        .spawn()
        .expect("Failed to start gRPC service");

    // Busy-poll the server's port instead of sleeping a fixed amount of time: a fixed
    // sleep is either a wasted delay (server was ready sooner) or a race (server is
    // slower to bind than the sleep, e.g. under full-suite load) -- confirmed as the
    // actual cause of this test's own pre-existing flakiness under `cargo test
    // --workspace` (it went from failing roughly 3 of every 4 full-suite runs to 8/8
    // passes once this replaced the old fixed 500ms sleep). Exponential backoff
    // starting at 1ms (instead of a fixed poll interval) so a fast-starting server
    // isn't penalized with wasted sleeps, up to a 60s total budget; each sleep is
    // clamped to the remaining budget so the loop can't overshoot it.
    let start = std::time::Instant::now();
    let max_wait = Duration::from_secs(60);
    let mut backoff = Duration::from_millis(1);
    loop {
        if std::net::TcpStream::connect("[::1]:50051").is_ok() {
            break;
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed < max_wait,
            "gRPC service never started listening on [::1]:50051 within 60s"
        );
        thread::sleep(backoff.min(max_wait - elapsed));
        backoff = (backoff * 2).min(max_wait);
    }

    // Run the crush gRPC client against the server: send a fully populated `Blob` message
    // to the streaming `Mirror` RPC and verify every field comes back unchanged. See
    // tests/grpc/mirror.crush for the actual test logic; it signals pass/fail via
    // crush:exit's own status (force=$true, since the gRPC client's streaming call can
    // leave its own internal jobs registered even after closing the connection, which
    // would otherwise make crush:exit refuse to run at all).
    let output = Command::new("./target/debug/crush")
        .args(&["tests/grpc/mirror.crush"])
        .output()
        .expect("failed to execute process");

    let _ = server.kill();
    let _ = server.wait();

    assert_eq!(
        output.status.code(),
        Some(0),
        "gRPC Mirror round-trip test did not pass.\nStdout:\n{}\nStderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn test_remote_host_file() {
    // remote:host:list/remote:host:remove (src/builtins/remote.rs) never connect
    // anywhere -- they just read/rewrite a known_hosts file -- so a static fixture is
    // enough; unlike test_remote_ssh below, no live server is involved. Three
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

    let output = Command::new("./target/debug/crush")
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

#[test]
fn test_remote_ssh() {
    let run = escargot::CargoBuild::new()
        .bin("ssh-service")
        .package("ssh-service") // Name of the sub-crate
        .run()
        .expect("Failed to build ssh-service binary");

    let mut server = Command::new(run.path())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start ssh test server");

    // ssh-service generates a fresh host key every run and prints its known_hosts-
    // format line on startup, so the two sides never need to agree on hardcoded key
    // material -- read it back to build the known_hosts fixtures below.
    let stdout = server.stdout.take().expect("piped stdout");
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("failed to read ssh-service startup line");
    let known_hosts_line = line
        .trim()
        .strip_prefix("KNOWN_HOSTS_LINE:")
        .expect("unexpected ssh-service startup output")
        .to_string();

    // Give it a moment to actually bind and start listening (which happens just after
    // the print above).
    thread::sleep(Duration::from_millis(300));

    let tmp = std::env::temp_dir();
    let good_hosts = tmp.join("crush_test_ssh_known_hosts_good");
    let mismatch_hosts = tmp.join("crush_test_ssh_known_hosts_mismatch");
    let empty_hosts = tmp.join("crush_test_ssh_known_hosts_empty");
    let allow_hosts = tmp.join("crush_test_ssh_known_hosts_allow");

    fs::write(&good_hosts, format!("{}\n", known_hosts_line)).unwrap();

    // Corrupt the first few base64 characters right after "<host> <algo> " -- still
    // valid base64 of the same length, so this is a real CheckResult::Mismatch rather
    // than a key-parse failure.
    let mismatched_line = {
        let mut parts = known_hosts_line.splitn(3, ' ');
        let host = parts.next().unwrap();
        let algo = parts.next().unwrap();
        let key = parts.next().unwrap();
        format!("{} {} XXXXXX{}", host, algo, &key[6..])
    };
    fs::write(&mismatch_hosts, format!("{}\n", mismatched_line)).unwrap();
    fs::write(&empty_hosts, "").unwrap();
    fs::write(&allow_hosts, "").unwrap();

    let run_crush = |script: &str, host_file: &Path| -> std::process::Output {
        Command::new("./target/debug/crush")
            .args(&[script])
            .env("CRUSH_TEST_SSH_HOSTS", host_file)
            .output()
            .expect("failed to execute process")
    };

    // Happy path: remote:exec and remote:pexec against a host_file that matches the
    // server's actual key.
    let out = run_crush("tests/remote/ssh_exec.crush", &good_hosts);
    assert_eq!(
        out.status.code(),
        Some(0),
        "ssh_exec.crush failed.\nStdout:\n{}\nStderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // Host key mismatch must be a hard error naming the mismatch, not some other,
    // coincidental failure.
    let out = run_crush("tests/remote/ssh_exec_mismatch.crush", &mismatch_hosts);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected a host-key-mismatch error to abort the script"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("mismatch"),
        "expected a host-key mismatch error, got:\n{}",
        stderr
    );

    // ignore_host_file=$true must skip verification entirely -- confirm by using the
    // same mismatched known_hosts file that made the test above fail: the connection
    // must succeed here, which can only happen if the check was actually skipped.
    let out = run_crush("tests/remote/ssh_exec_ignore_host_file.crush", &mismatch_hosts);
    assert_eq!(
        out.status.code(),
        Some(0),
        "ssh_exec_ignore_host_file.crush failed.\nStdout:\n{}\nStderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // A host missing from known_hosts, without allow_not_found, must also be a hard
    // error.
    let out = run_crush("tests/remote/ssh_exec_notfound.crush", &empty_hosts);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected a missing-known-host error to abort the script"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("known host"),
        "expected a missing-from-known-hosts error, got:\n{}",
        stderr
    );

    // Wrong password must also be a hard error.
    let out = run_crush("tests/remote/ssh_exec_wrongpassword.crush", &good_hosts);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected an authentication error to abort the script"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("auth"),
        "expected an authentication error, got:\n{}",
        stderr
    );

    // allow_not_found=$true against an empty known_hosts file should succeed *and* pin
    // the newly seen key into the file, rather than erroring.
    let out = run_crush("tests/remote/ssh_exec_allow_not_found.crush", &allow_hosts);
    assert_eq!(
        out.status.code(),
        Some(0),
        "ssh_exec_allow_not_found.crush failed.\nStdout:\n{}\nStderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let pinned = fs::read_to_string(&allow_hosts).unwrap();
    let real_key = known_hosts_line.split(' ').nth(2).unwrap();
    assert!(
        pinned.contains(real_key),
        "allow_not_found should have pinned the server's actual key into the known_hosts file, got:\n{}",
        pinned,
    );

    let _ = server.kill();
    let _ = server.wait();
}

test_finder!();
