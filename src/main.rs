mod app;
mod icons;
mod link;
mod model;
pub use feather_core::net;
mod theme;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("feather-chat {}", net::VERSION);
        return Ok(());
    }

    // Headless hub mode: `feather-chat --serve --port 8777`. The dedicated
    // `feather-hub` program does the same job with a much smaller download;
    // this is here so a single file can still host a space.
    if args.iter().any(|a| a == "--serve") {
        let mut cfg = net::HubConfig::from_env();
        cfg.strict_port = true;
        if let Some(i) = args.iter().position(|a| a == "--port") {
            if let Some(p) = args.get(i + 1).and_then(|p| p.parse::<u16>().ok()) {
                cfg.port = p;
            }
        }
        if let Some(i) = args.iter().position(|a| a == "--data") {
            if let Some(path) = args.get(i + 1) {
                cfg.data_path = std::path::PathBuf::from(path);
            }
        }
        if let Some(i) = args.iter().position(|a| a == "--bind") {
            if let Some(bind) = args.get(i + 1) {
                cfg.bind = bind.clone();
            }
        }
        if args.iter().any(|a| a == "--closed") {
            cfg.signup_open = false;
        }
        match net::Hub::start_with(cfg) {
            Ok(hub) => {
                println!("Feather hub {} listening on port {}", net::VERSION, hub.port);
                println!("Data file: {}", hub.data_path.display());
                println!("Phone page on this network: {}", net::hub_url(hub.port));
                println!("Health check: http://127.0.0.1:{}/api/health", hub.port);
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
            }
            Err(e) => {
                eprintln!("Could not start the hub: {e}");
                std::process::exit(1);
            }
        }
    }
    let screenshot_path = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .cloned();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1240.0, 800.0])
        .with_min_inner_size([980.0, 640.0])
        .with_title("Feather Chat");

    if screenshot_path.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::viewport::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    {
        native_options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "Feather Chat",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::FeatherApp::new(cc, screenshot_path)))),
    )
}
