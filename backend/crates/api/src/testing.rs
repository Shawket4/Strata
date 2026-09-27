//! Test support (feature `test-support`): an in-process HTTP server on an ephemeral port and a
//! demo router that exercises the wire machinery end to end. Not part of production builds.

pub mod demo;

use std::net::SocketAddr;
use std::sync::mpsc;
use std::thread::JoinHandle;

use actix_web::dev::ServerHandle;
use actix_web::{App, HttpServer, web};

/// A real Actix server on `127.0.0.1:<ephemeral>` running in its own thread and runtime, so
/// tests may use any async runtime (or paused tokio time) on their side. Stopped on drop.
#[derive(Debug)]
pub struct TestServer {
    addr: SocketAddr,
    handle: ServerHandle,
    thread: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Starts a single-worker server whose `App` is configured by `configure`.
    pub fn start<F>(configure: F) -> std::io::Result<Self>
    where
        F: Fn(&mut web::ServiceConfig) + Send + Clone + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let result = actix_rt::System::new().block_on(async move {
                let server = HttpServer::new(move || App::new().configure(configure.clone()))
                    .workers(1)
                    .bind(("127.0.0.1", 0))?;
                let addr = server.addrs().first().copied();
                let server = server.run();
                // The receiver only disappears if `start` already returned an error.
                let _ = tx.send(addr.map(|a| (a, server.handle())));
                server.await
            });
            if let Err(err) = result {
                tracing::error!(error = %err, "test server failed");
            }
        });
        let (addr, handle) = rx
            .recv()
            .ok()
            .flatten()
            .ok_or_else(|| std::io::Error::other("test server did not start"))?;
        Ok(Self {
            addr,
            handle,
            thread: Some(thread),
        })
    }

    /// `http://127.0.0.1:<port>` (no trailing slash).
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Socket address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // `stop` sends the command immediately; the returned future only awaits completion.
        drop(self.handle.stop(false));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
