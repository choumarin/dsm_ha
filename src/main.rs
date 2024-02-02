use clap::Parser;
use daemonizr::{Daemonizr, DaemonizrError, Stderr, Stdout};
use log::*;
use simple_logger::SimpleLogger;
use std::io::Write;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::Command;
use std::time::Duration;
use std::{path::PathBuf, process::exit};
use std::{thread, time};

/// Simple service to reattach USB to a VM on DSM
#[derive(Parser, Debug)]
struct Args {
    /// Virtual Machine uuid
    #[arg(long)]
    machine: String,

    /// USB device info file
    #[arg(long)]
    usb_file: String,

    /// Port to listen on
    #[arg(long)]
    port: u16,
}

fn handle_client(mut stream: TcpStream, machine: &str, usb_file: &str) {
    info!("Got a connection, reattaching usb");
    let out = Command::new("virsh")
        .args(["detach-device", machine, "--file", usb_file])
        .output()
        .expect("failed to detach usb");
    debug!("{out:?}");
    if !out.stderr.is_empty() {
        stream.write_all("ohnoes".as_bytes()).unwrap_or_default();
    } else {
        thread::sleep(Duration::from_secs(5));
        let out = Command::new("virsh")
            .args(["attach-device", machine, "--file", usb_file])
            .output()
            .expect("failed to attach usb");
        debug!("{out:?}");
        if !out.stderr.is_empty() {
            stream.write_all("ohnoes".as_bytes()).unwrap_or_default();
        } else {
            stream.write_all("kthxbye".as_bytes()).unwrap_or_default();
        }
    }
    stream.shutdown(Shutdown::Both).unwrap_or(());
    // This forces 60s before accepting the next connection.
    thread::sleep(time::Duration::from_secs(60));
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    match Daemonizr::new()
        .pidfile(PathBuf::from("dmnzr.pid"))
        .stdout(Stdout::Redirect(PathBuf::from("dmnzr.out")))
        .stderr(Stderr::Redirect(PathBuf::from("dmnzr.err")))
        .spawn()
    {
        Err(DaemonizrError::AlreadyRunning) => {
            /* search for the daemon's PID  */
            match Daemonizr::new()
                .pidfile(PathBuf::from("dmnzr.pid"))
                .search()
            {
                Err(x) => eprintln!("error: {}", x),
                Ok(pid) => {
                    eprintln!("another daemon with pid {} is already running", pid);
                    exit(1);
                }
            };
        }
        Err(e) => eprintln!("DaemonizrError: {}", e),
        Ok(()) => { /* We are in daemon process now */ }
    };

    SimpleLogger::new().init().unwrap();

    info!("Starting listener");
    let listener = TcpListener::bind(("0.0.0.0", args.port))?;
    info!("Listener started");
    // accept connections and process them serially
    for stream in listener.incoming() {
        handle_client(stream?, &args.machine, &args.usb_file);
    }
    info!("Exiting");
    Ok(())
}
