// A minimal, test-only SSH server used by `tests/system.rs` to exercise `remote:exec`/
// `remote:pexec` end to end. On every accepted "exec" channel it spawns the real local
// `crush --pup` binary and pipes the SSH channel's data straight to and from that child
// process's stdin/stdout -- so what's actually being tested is our own ssh2-based client
// code (host key checking, auth) driving a real pup protocol round trip, not a
// reimplementation of that protocol inside the test server.
//
// Deliberately NOT hardened: fixed, well-known credentials, a fixed host key, no rate
// limiting. This binary only ever binds 127.0.0.1 and only exists for the test suite --
// see `src/builtins/remote.rs` for the production client code this exercises.
//
// The host key is a fixed, committed seed rather than freshly generated per run: there's
// no security reason to rotate it (this server is never reachable from outside the test
// run, and is deliberately unhardened already), and a fixed key lets the known_hosts
// fixtures the test suite verifies against (see tests/ssh_exec*.crush) be plain,
// self-contained literals instead of something Rust has to generate and hand to them
// via stdout on every run. Its corresponding known_hosts line is:
//   127.0.0.1 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPABgyOJckbS7f/mCs1K+zm7WApZ5fxPvES1xyrKWqef
const HOST_KEY_SEED: [u8; 32] = [
    0x69, 0xbf, 0x27, 0x57, 0x10, 0xc0, 0x98, 0xc0, 0x92, 0x5d, 0x82, 0x11, 0xa7, 0xeb, 0x65, 0x91,
    0x69, 0x35, 0x0c, 0xfa, 0x32, 0xcc, 0xfd, 0xa1, 0x7d, 0x91, 0x9a, 0x29, 0x84, 0xd9, 0x34, 0x85,
];

use async_trait::async_trait;
use ed25519_dalek::SigningKey;
use russh::keys::key;
use russh::server::{Auth, Config, Handler, Msg, Server as _, Session};
use russh::{Channel, ChannelId, CryptoVec};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{ChildStdin, Command as TokioCommand};
use tokio::sync::Mutex;

/// Must match the `username=`/`password=` arguments `tests/ssh_exec*.crush` passes to
/// `remote:exec`/`remote:pexec`.
const TEST_USER: &str = "crushtest";
const TEST_PASSWORD: &str = "crushtest-password";

const BIND_ADDR: &str = "127.0.0.1";
const BIND_PORT: u16 = 2849;

// See dns-service/src/main.rs's install_graceful_shutdown for why this is needed: a
// server killed by a plain SIGTERM never gets to flush coverage-instrumentation data
// via Rust's normal atexit path, so it always reports 0% coverage under `cargo
// llvm-cov` regardless of what actually ran.
fn install_graceful_shutdown() {
    std::thread::spawn(|| {
        let mut signals = signal_hook::iterator::Signals::new([signal_hook::consts::SIGTERM])
            .expect("failed to register SIGTERM handler");
        signals.forever().next();
        std::process::exit(0);
    });
}

#[tokio::main]
async fn main() {
    install_graceful_shutdown();

    let crush_bin = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./target/debug/crush".to_string());

    let key_pair = key::KeyPair::Ed25519(SigningKey::from_bytes(&HOST_KEY_SEED));

    let config = Arc::new(Config {
        keys: vec![key_pair],
        ..Default::default()
    });

    let mut server = SshTestServer {
        crush_bin: Arc::new(crush_bin),
    };
    server
        .run_on_address(config, (BIND_ADDR, BIND_PORT))
        .await
        .expect("ssh test server failed");
}

#[derive(Clone)]
struct SshTestServer {
    crush_bin: Arc<String>,
}

impl russh::server::Server for SshTestServer {
    type Handler = ClientHandler;

    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> ClientHandler {
        ClientHandler {
            crush_bin: self.crush_bin.clone(),
            stdins: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

struct ClientHandler {
    crush_bin: Arc<String>,
    stdins: Arc<Mutex<HashMap<ChannelId, ChildStdin>>>,
}

#[async_trait]
impl Handler for ClientHandler {
    type Error = anyhow::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        if user == TEST_USER && password == TEST_PASSWORD {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::Reject {
                proceed_with_methods: None,
            })
        }
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // The crush client only ever sends exactly this; reject anything else outright
        // rather than silently running it.
        if data != b"crush --pup" {
            session.channel_failure(channel);
            return Ok(());
        }

        let mut child = match TokioCommand::new(&*self.crush_bin)
            .arg("--pup")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                session.channel_failure(channel);
                return Ok(());
            }
        };

        let stdin = child.stdin.take().expect("piped stdin");
        let mut stdout = child.stdout.take().expect("piped stdout");
        self.stdins.lock().await.insert(channel, stdin);

        let handle = session.handle();
        tokio::spawn(async move {
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if handle.data(channel, CryptoVec::from_slice(&buf[..n])).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = child.wait().await;
            let _ = handle.eof(channel).await;
            let _ = handle.exit_status_request(channel, 0).await;
            let _ = handle.close(channel).await;
        });

        session.channel_success(channel);
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(stdin) = self.stdins.lock().await.get_mut(&channel) {
            let _ = stdin.write_all(data).await;
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Dropping the ChildStdin closes its write half, signaling EOF to `crush --pup` --
        // exactly what the crush client's own `channel.send_eof()` expects the far end
        // to observe.
        self.stdins.lock().await.remove(&channel);
        Ok(())
    }
}
