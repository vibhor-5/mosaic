use std::process::{Command, Child};
use std::time::Duration;
use std::thread;
use std::os::unix::net::UnixStream;
use std::io::Write;

fn start_daemon() -> Child {
    // Start the mosaic daemon
    let child = Command::new("cargo")
        .args(&["run", "--bin", "mosaic"])
        .spawn()
        .expect("Failed to start mosaic daemon");

    // Wait for the socket to be created
    thread::sleep(Duration::from_secs(3));
    child
}

fn send_ipc_message(msg: &str) -> std::io::Result<()> {
    let mut stream = UnixStream::connect("/tmp/mosaic.sock")?;
    stream.write_all(msg.as_bytes())?;
    Ok(())
}

#[test]
fn test_e2e_daemon_lifecycle() {
    // 1. Start daemon
    let mut daemon = start_daemon();

    // 2. Test IPC communication
    let res = send_ipc_message("bsp");
    assert!(res.is_ok(), "Failed to send 'bsp' IPC message");

    thread::sleep(Duration::from_millis(500));

    let res = send_ipc_message("monocle");
    assert!(res.is_ok(), "Failed to send 'monocle' IPC message");

    thread::sleep(Duration::from_millis(500));

    // 3. Gracefully or forcefully kill the daemon
    // Sending an unrecognized message or just killing it
    daemon.kill().expect("Failed to kill daemon");
    daemon.wait().expect("Failed to wait on daemon");
}
