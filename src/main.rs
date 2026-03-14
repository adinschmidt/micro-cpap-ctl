mod configure;
mod device;
mod info;
mod model;
mod monitor;
mod session;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "micro-cpap-ctl", about = "Micro CPAP command-line control tool")]
struct Cli {
    /// Serial port path
    #[arg(short, long, default_value = "/dev/ttyUSB0", global = true)]
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

    /// View session statistics
    Session {
        /// Session offset: 1 = most recent, 2 = second-most recent, etc.
        #[arg(short = 'n', long, default_value_t = 1)]
        offset: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut dev = device::Device::open(&cli.port)?;
    println!("Connected to {}", cli.port);

    match cli.command {
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
