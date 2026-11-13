//! The headless Feather hub. This is the program a server runs: it listens on
//! a port, serves the phone page and stores everything in one JSON file. It
//! never opens a window and has no drawing code linked in.

use feather_core::net::{self, Hub, HubConfig};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("feather-hub {}", net::VERSION);
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        help();
        return;
    }

    let mut cfg = HubConfig::from_env();
    // A supervised server should fail loudly rather than move to another port.
    cfg.strict_port = true;

    if let Some(v) = flag_value(&args, "--bind") {
        cfg.bind = v;
    }
    if let Some(v) = flag_value(&args, "--port") {
        match v.parse::<u16>() {
            Ok(p) => cfg.port = p,
            Err(_) => fail(&format!("--port needs a number, not {v:?}.")),
        }
    }
    if let Some(v) = flag_value(&args, "--data") {
        cfg.data_path = PathBuf::from(v);
    }
    if args.iter().any(|a| a == "--open") {
        cfg.signup_open = true;
    } else if args.iter().any(|a| a == "--closed") {
        cfg.signup_open = false;
    }

    // Container health check: ask the running hub for its own report.
    if args.iter().any(|a| a == "--health") {
        std::process::exit(health(cfg.port));
    }

    let data = cfg.data_path.display().to_string();
    let bind = cfg.bind.clone();
    let signup = cfg.signup_open;

    match Hub::start_with(cfg) {
        Ok(hub) => {
            println!("Feather hub {}", net::VERSION);
            println!("Listening on {bind}:{}", hub.port);
            println!("Data file: {data}");
            println!(
                "New accounts: {}",
                if signup { "open to anyone with the address" } else { "closed" }
            );
            if bind == "0.0.0.0" || bind == "::" {
                println!("Phone page (same network): http://{}", net::share_host(hub.port));
            }
            println!("Health check: http://127.0.0.1:{}/api/health", hub.port);
            println!("Ready. Press Ctrl+C to stop.");
            loop {
                std::thread::sleep(Duration::from_secs(3600));
            }
        }
        Err(e) => fail(&e),
    }
}

fn health(port: u16) -> i32 {
    let host = std::env::var("FEATHER_HEALTH_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let Ok(addr) = format!("{host}:{port}").parse::<std::net::SocketAddr>() else {
        eprintln!("Bad health address.");
        return 2;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(3)) else {
        eprintln!("No answer from the hub on {addr}.");
        return 1;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let req = "GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    if stream.write_all(req.as_bytes()).is_err() {
        eprintln!("Could not ask the hub for its health.");
        return 1;
    }
    let mut text = String::new();
    if stream.read_to_string(&mut text).is_err() || !text.starts_with("HTTP/1.1 200") {
        eprintln!("The hub did not report itself healthy.");
        return 1;
    }
    if let Some((_, body)) = text.split_once("\r\n\r\n") {
        println!("{body}");
    }
    0
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn help() {
    println!(
        "feather-hub {}\n\
         \n\
         A tiny chat hub: one JSON file, one port, no database.\n\
         \n\
         Usage:\n\
           feather-hub [--port 8777] [--bind 0.0.0.0] [--data /data/hub.json]\n\
                       [--closed] [--health]\n\
         \n\
         Settings can also come from the environment:\n\
           FEATHER_PORT    port to listen on (default 8777)\n\
           FEATHER_BIND    address to listen on (default 0.0.0.0)\n\
           FEATHER_DATA    data file (default hub.json in the user's data folder)\n\
           FEATHER_SIGNUP  open (default) or closed\n\
         \n\
         Put an HTTPS proxy in front for the internet. See DEPLOY.md.",
        net::VERSION
    );
}

fn fail(message: &str) -> ! {
    eprintln!("Feather hub could not start: {message}");
    std::process::exit(1);
}
