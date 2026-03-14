mod configure;
mod device;
mod info;
mod model;
mod monitor;
mod session;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cpap", about = "Micro CPAP command-line control tool")]
struct Cli {
    /// Serial port path (auto-detected if omitted)
    #[arg(short, long, global = true)]
    port: Option<String>,

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

    /// Scan serial ports to find a connected CPAP device
    Detect,

    /// List available serial ports
    ListPorts,

    /// View session statistics
    Session {
        /// Session offset: 1 = most recent, 2 = second-most recent, etc.
        #[arg(short = 'n', long, default_value_t = 1)]
        offset: usize,
    },
}

/// Resolve the serial port: use --port if given, otherwise probe USB serial
/// devices to find a CPAP that actually responds.
fn resolve_port(explicit: Option<&str>) -> Result<String> {
    if let Some(p) = explicit {
        return Ok(p.to_string());
    }

    eprintln!("Scanning for CPAP device...");
    if let Some((port, code)) = device::Device::detect() {
        eprintln!("Found device (type {code}) on {port}");
        return Ok(port);
    }

    bail!(
        "No CPAP device found. Is it plugged in?\n\
         Run `cpap list-ports` to see available serial ports,\n\
         then specify one with `cpap --port <device> <command>`."
    )
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // These don't need a device connection
        Command::ListPorts => {
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
            Ok(())
        }

        Command::Detect => {
            eprintln!("Scanning all USB serial ports...");
            match device::Device::detect() {
                Some((port, code)) => {
                    println!("{port}");
                    eprintln!("Device type: {code}");
                    Ok(())
                }
                None => {
                    bail!("No CPAP device found on any USB serial port.");
                }
            }
        }

        // Everything else needs a device
        command => {
            let port = resolve_port(cli.port.as_deref())?;
            let mut dev = device::Device::open(&port)?;
            println!("Connected to {port}");

            match command {
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
                Command::ListPorts | Command::Detect => unreachable!(),
            }
        }
    }
}
