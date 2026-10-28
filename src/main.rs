mod app;
mod ffi;
mod model;
mod storage;

use std::path::PathBuf;

#[derive(Default)]
pub struct Options {
    pub data_dir: Option<PathBuf>,
    pub open: Option<usize>,
    pub qa_report: Option<PathBuf>,
    pub snapshot: Option<PathBuf>,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = Options::default();
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                println!("Messages {}\n\nNative Rust / GTK / WebKitGTK app\n\n  --data-dir PATH      Separate app data directory\n  --open-service ID    Open whatsapp, telegram, instagram, linkedin, or gmail\n  --doctor             Show native runtime versions\n  --version            Print version\n\nClose the window or use its power button to quit. No tray or daemon.", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Some("--version" | "-V") => {
                println!(
                    "relay {} (native GTK / WebKitGTK)",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            Some("--doctor") => {
                unsafe {
                    println!("Native Rust application\nGTK {}.{}.{}\nWebKitGTK {}.{}.{} (API 4.1)\nFirefox: not used\nBrowser extension: not used\nBackground daemon: none",
                        ffi::gtk_get_major_version(), ffi::gtk_get_minor_version(), ffi::gtk_get_micro_version(),
                        ffi::webkit_get_major_version(), ffi::webkit_get_minor_version(), ffi::webkit_get_micro_version());
                }
                return Ok(());
            }
            Some("--data-dir") => {
                options.data_dir = Some(args.next().ok_or("--data-dir needs a path")?.into())
            }
            Some("--open-service") => {
                let id = args.next().ok_or("--open-service needs an ID")?;
                options.open = Some(
                    model::SERVICES
                        .iter()
                        .position(|service| id == service.id)
                        .ok_or("Unknown service ID")?,
                );
            }
            Some("--self-test") => {
                options.qa_report =
                    Some(args.next().ok_or("--self-test needs a report path")?.into())
            }
            Some("--snapshot") => {
                options.snapshot = Some(args.next().ok_or("--snapshot needs an image path")?.into())
            }
            _ => return Err(format!("Unknown option: {}", arg.to_string_lossy()).into()),
        }
    }
    if options.qa_report.is_some() && options.data_dir.is_none() {
        return Err("Self-test requires a separate --data-dir.".into());
    }
    if options.snapshot.is_some() && options.qa_report.is_none() {
        return Err("--snapshot is only used with --self-test.".into());
    }
    app::run(options)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Messages: {error}");
        std::process::exit(1);
    }
}
