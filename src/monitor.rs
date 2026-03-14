use anyhow::Result;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::device::Device;
use crate::model::LiveEvent;

const MAX_EVENTS: usize = 8;
const EVENT_POLL_INTERVAL: u32 = 5; // check for new events every N cycles

/// Enable ANSI escape code support on Windows 10+.
#[cfg(target_os = "windows")]
fn enable_virtual_terminal() {
    use std::os::windows::io::AsRawHandle;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    unsafe {
        let handle = std::io::stdout().as_raw_handle();
        let mut mode: u32 = 0;
        // GetConsoleMode / SetConsoleMode from kernel32
        extern "system" {
            fn GetConsoleMode(h: *mut std::ffi::c_void, mode: *mut u32) -> i32;
            fn SetConsoleMode(h: *mut std::ffi::c_void, mode: u32) -> i32;
        }
        if GetConsoleMode(handle as *mut _, &mut mode) != 0 {
            let _ = SetConsoleMode(handle as *mut _, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn enable_virtual_terminal() {}

fn bar(value: f64, max_val: f64, width: usize) -> String {
    let clamped = value.clamp(0.0, max_val);
    let filled = (clamped / max_val * width as f64) as usize;
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(width - filled))
}

pub fn run(dev: &mut Device, interval: f64) -> Result<()> {
    enable_virtual_terminal();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::SeqCst))?;

    // Bootstrap event tracking — skip events already in the log.
    let header = dev.read_device_info()?;
    let mut last_event_count = header.events_in_queue;
    let base_addr = dev.read_event_data_address()?;
    let mut event_addr = base_addr + last_event_count * 5;
    let mut recent: VecDeque<LiveEvent> = VecDeque::with_capacity(MAX_EVENTS);
    let mut poll_counter = 0u32;
    let mut samples = 0u64;

    // Hide cursor.
    print!("\x1b[?25l");
    io::stdout().flush()?;

    let result = (|| -> Result<()> {
        while running.load(Ordering::SeqCst) {
            let mon = dev.read_monitor().ok();
            let flow = dev.read_flow().ok();
            let sensor = dev.read_pressure_sensor().ok();
            samples += 1;
            poll_counter += 1;

            // Poll for new compliance events periodically.
            if poll_counter >= EVENT_POLL_INTERVAL {
                poll_counter = 0;
                if let Ok(hdr) = dev.read_device_info() {
                    if hdr.events_in_queue > last_event_count {
                        let num_new = hdr.events_in_queue - last_event_count;
                        if let Ok((evts, new_addr)) = dev.fetch_live_events(event_addr, num_new) {
                            for ev in evts {
                                if recent.len() >= MAX_EVENTS {
                                    recent.pop_front();
                                }
                                recent.push_back(ev);
                            }
                            event_addr = new_addr;
                        }
                        last_event_count = hdr.events_in_queue;
                    }
                }
            }

            // ── Render ───────────────────────────────────────────────
            print!("\x1b[2J\x1b[H"); // clear screen, cursor to top

            let sep = "=".repeat(50);
            println!("\x1b[1m{sep}\x1b[0m");
            println!("\x1b[1m  Micro CPAP — Live Monitor\x1b[0m");
            println!("\x1b[1m{sep}\x1b[0m");
            println!();

            if let Some(m) = &mon {
                println!("  Mode:               \x1b[1;36m{}\x1b[0m", m.mode);
                println!("  Pressure Goal:      {:6.1} cmH2O  {}", m.pressure_goal, bar(m.pressure_goal, 20.0, 30));
                println!("  Measured Pressure:  {:6.1} cmH2O  {}", m.measured_pressure, bar(m.measured_pressure, 20.0, 30));
                println!("  Lung Flow:          {:6.1} L/min  {}", m.lung_flow, bar(m.lung_flow.abs(), 60.0, 30));
                println!("  Leak:               {:6.1} L/min  {}", m.leak, bar(m.leak, 60.0, 30));
            } else {
                println!("  Monitor:            \x1b[31mno data\x1b[0m");
                for _ in 0..4 { println!(); }
            }

            println!();

            if let Some(f) = &flow {
                println!("  Hose Flow:          {:6.1} L/min", f.hose_flow);
                println!("  Baseline Flow:      {:6.1} L/min", f.baseline_flow);
            } else {
                println!("  Flow:               \x1b[31mno data\x1b[0m");
                println!();
            }

            match sensor {
                Some(s) => println!("  Pressure Sensor:    {:6.1} cmH2O", s),
                None => println!("  Pressure Sensor:    \x1b[31mno data\x1b[0m"),
            }

            println!();
            println!("\x1b[1m--- Recent Events ({last_event_count} total) ---\x1b[0m");
            if recent.is_empty() {
                println!("  \x1b[2m(none yet this session)\x1b[0m");
            } else {
                for ev in &recent {
                    println!("{ev}");
                }
            }

            println!();
            let now = chrono::Local::now().format("%H:%M:%S");
            println!("\x1b[2m  Sample #{samples}  |  {now}  |  Ctrl+C to stop\x1b[0m");

            io::stdout().flush()?;
            std::thread::sleep(Duration::from_secs_f64(interval));
        }
        Ok(())
    })();

    // Restore cursor.
    print!("\x1b[?25h\n");
    io::stdout().flush()?;
    println!("Stopped.");
    result
}
