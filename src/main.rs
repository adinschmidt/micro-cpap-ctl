mod configure;
mod device;
mod info;
mod model;
mod monitor;
mod session;

use anyhow::Result;
use clap::{Parser, Subcommand};

fn default_port() -> String {
    // macOS: try to find a matching serial device automatically
    #[cfg(target_os = "macos")]
    {
        for pattern in &["/dev/tty.usbserial-*", "/dev/tty.usbmodem-*"] {
            if let Ok(paths) = glob::glob(pattern) {
                for entry in paths.flatten() {
                    return entry.to_string_lossy().into_owned();
                }
            }
        }
        return "/dev/tty.usbserial-0".to_string();
    }

    // Windows
    #[cfg(target_os = "windows")]
    {
        return "COM3".to_string();
    }

    // Linux (default)
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return "/dev/ttyUSB0".to_string();
    }
}

#[derive(Parser)]
#[command(name = "micro-cpap-ctl", about = "Micro CPAP command-line control tool")]
struct Cli {
    /// Serial port path (auto-detected if omitted)
    #[arg(short, long, default_value_t = default_port(), global = true)]
    port: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Live therapy data monitor
    Monitor {
        /// Poll interval in seconds
        #[arg(short, long, default_value_t = 0.5)]
        interval: f64,
    },

    /// Read device info and configuration
    Info,

    /// Modify device configuration
    Set {
        /// Therapy pressure (cmH2O, 4.0–20.0)
        #[arg(long)]
        pressure: Option<f64>,

        /// Ramp starting pressure (cmH2O, 4.0–20.0)
        #[arg(long)]
        ramp_pressure: Option<f64>,

        /// Ramp duration in minutes (0 = off, 5–45)
        #[arg(long)]
        ramp_time: Option<i32>,

        /// Min therapy pressure — APAP only (4.0–20.0)
        #[arg(long)]
        min_pressure: Option<f64>,

        /// Max therapy pressure — APAP only (4.0–20.0)
        #[arg(long)]
        max_pressure: Option<f64>,

        /// EZEX level — APAP/EZEX only (0–3)
        #[arg(long)]
        ezex: Option<i32>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// List available serial ports
    ListPorts,

    /// View session statistics
    Session {
        /// Session offset: 1 = most recent, 2 = second-most recent, etc.
        #[arg(short = 'n', long, default_value_t = 1)]
        offset: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // list-ports doesn't need a connection
    if matches!(cli.command, Command::ListPorts) {
        let ports = serialport::available_ports()?;
        if ports.is_empty() {
            println!("No serial ports found.");
        } else {
            println!("Available serial ports:");
            for p in &ports {
                let info = match &p.port_type {
                    serialport::SerialPortType::UsbPort(usb) => {
                        let prod = usb.product.as_deref().unwrap_or("Unknown");
                        let mfg = usb.manufacturer.as_deref().unwrap_or("Unknown");
                        format!("  USB — {} ({})", prod, mfg)
                    }
                    serialport::SerialPortType::BluetoothPort => "  Bluetooth".to_string(),
                    serialport::SerialPortType::PciPort => "  PCI".to_string(),
                    _ => "".to_string(),
                };
                println!("  {}{}", p.port_name, info);
            }
        }
        return Ok(());
    }

    let mut dev = device::Device::open(&cli.port)?;
    println!("Connected to {}", cli.port);

    match cli.command {
        Command::ListPorts => unreachable!(),
        Command::Monitor { interval } => monitor::run(&mut dev, interval),
        Command::Info => info::run(&mut dev),
        Command::Set {
            pressure,
            ramp_pressure,
            ramp_time,
            min_pressure,
            max_pressure,
            ezex,
            yes,
        } => configure::run(
            &mut dev,
            configure::SetArgs {
                pressure,
                ramp_pressure,
                ramp_time,
                min_pressure,
                max_pressure,
                ezex,
                yes,
            },
        ),
        Command::Session { offset } => session::run(&mut dev, offset),
    }
}
