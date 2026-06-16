//! # mosaic-msg - CLI client for the Mosaic tiling window manager
//!
//! Sends commands to the running Mosaic daemon via its Unix socket.
//!
//! ## Usage
//!
//! ```sh
//! mosaic-msg focus east
//! mosaic-msg swap west
//! mosaic-msg layout monocle
//! mosaic-msg space 3
//! mosaic-msg move-to-space 2
//! mosaic-msg toggle float
//! mosaic-msg rotate tree
//! mosaic-msg equalize tree
//! mosaic-msg resize east 0.05
//! mosaic-msg query windows
//! mosaic-msg retile
//! mosaic-msg quit
//! ```

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/mosaic.sock";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: mosaic-msg <command> [args...]");
        eprintln!();
        eprintln!("Commands:");
        eprintln!("  focus <direction>              Focus window in direction (north|south|east|west)");
        eprintln!("  swap <direction>               Swap focused window with neighbor");
        eprintln!("  layout <mode>                  Set layout (bsp|monocle|master-stack)");
        eprintln!("  space <number>                 Switch to space N");
        eprintln!("  move-to-space <number>         Move focused window to space N");
        eprintln!("  toggle <target>                Toggle property (float|fullscreen)");
        eprintln!("  rotate tree                    Rotate the BSP tree");
        eprintln!("  equalize tree                  Reset all split ratios to 50/50");
        eprintln!("  resize <direction> <delta>     Resize in direction by delta (0.0-1.0)");
        eprintln!("  query <target>                 Query state (windows|spaces|focused)");
        eprintln!("  retile                         Force retile current space");
        eprintln!("  quit                           Stop the Mosaic daemon");
        std::process::exit(1);
    }

    let command = args.join(" ");

    match UnixStream::connect(SOCKET_PATH) {
        Ok(mut stream) => {
            // Send the command
            if let Err(e) = writeln!(stream, "{}", command) {
                eprintln!("Error sending command: {}", e);
                std::process::exit(1);
            }
            stream.flush().ok();

            // Shutdown the write half so the server knows we're done
            stream.shutdown(std::net::Shutdown::Write).ok();

            // Read the response
            let reader = BufReader::new(&stream);
            for line in reader.lines() {
                match line {
                    Ok(response) => {
                        println!("{}", response);
                    }
                    Err(e) => {
                        eprintln!("Error reading response: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error: Could not connect to Mosaic daemon at {}", SOCKET_PATH);
            eprintln!("Is Mosaic running? ({})", e);
            std::process::exit(1);
        }
    }
}
