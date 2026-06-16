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

#![allow(dead_code)]
use clap::{Parser, Subcommand};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/mosaic.sock";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Focus window in direction (north|south|east|west)
    Focus { direction: String },
    /// Swap focused window with neighbor in direction
    Swap { direction: String },
    /// Set layout (bsp|monocle|master-stack)
    Layout { mode: String },
    /// Switch to space N
    Space { number: u64 },
    /// Move focused window to space N
    MoveToSpace { number: u64 },
    /// Toggle property (float|fullscreen)
    Toggle { target: String },
    /// Rotate the BSP tree
    Rotate { target: String },
    /// Reset all split ratios to 50/50
    Equalize { target: String },
    /// Resize in direction by delta (0.0-1.0)
    Resize { direction: String, delta: f64 },
    /// Query state (windows|spaces|focused)
    Query { target: String },
    /// Force retile current space
    Retile,
    /// Stop the Mosaic daemon
    Quit,
}

fn main() {
    let cli = Cli::parse();

    let cmd_str = match &cli.command {
        Commands::Focus { direction } => format!("focus {}", direction),
        Commands::Swap { direction } => format!("swap {}", direction),
        Commands::Layout { mode } => format!("layout {}", mode),
        Commands::Space { number } => format!("space {}", number),
        Commands::MoveToSpace { number } => format!("move-to-space {}", number),
        Commands::Toggle { target } => format!("toggle {}", target),
        Commands::Rotate { target } => format!("rotate {}", target),
        Commands::Equalize { target } => format!("equalize {}", target),
        Commands::Resize { direction, delta } => format!("resize {} {}", direction, delta),
        Commands::Query { target } => format!("query {}", target),
        Commands::Retile => "retile".to_string(),
        Commands::Quit => "quit".to_string(),
    };

    match UnixStream::connect(SOCKET_PATH) {
        Ok(mut stream) => {
            if let Err(e) = writeln!(stream, "{}", cmd_str) {
                eprintln!("Error sending command: {}", e);
                std::process::exit(1);
            }
            stream.flush().ok();
            stream.shutdown(std::net::Shutdown::Write).ok();

            let reader = BufReader::new(&stream);
            for line in reader.lines() {
                match line {
                    Ok(response) => println!("{}", response),
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
