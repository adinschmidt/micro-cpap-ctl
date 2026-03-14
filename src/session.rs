use anyhow::Result;

use crate::device::Device;
use crate::model::{Event, EventKind, SessionStats, format_hm};

/// Find the Nth-most-recent session in the event list.
/// Returns `(session_slice, session_number, total_sessions)`.
fn find_session(events: &[Event], offset: usize) -> (&[Event], usize, usize) {
    let starts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == EventKind::SessionStart)
        .map(|(i, _)| i)
        .collect();

    let total = starts.len();
    if total == 0 || offset < 1 || offset > total {
        return (&[], offset, total);
    }

    let start = starts[total - offset];
    let end = events[start..]
        .iter()
        .enumerate()
        .find(|(_, e)| e.kind == EventKind::SessionEnd)
        .map(|(i, _)| start + i);

    match end {
        Some(end) => (&events[start..=end], total - offset + 1, total),
        None => (&events[start..], total - offset + 1, total),
    }
}

fn display_stats(stats: &SessionStats, events: &[Event], num: usize, total: usize) {
    let status = if stats.in_progress { "  [SESSION IN PROGRESS]" } else { "" };

    println!("{}", "=".repeat(52));
    println!("  Session {num} of {total}{status}");
    println!("{}", "=".repeat(52));
    println!();
    println!("  Date:              {}", stats.start.format("%Y-%m-%d"));
    println!("  Start:             {}", stats.start.format("%H:%M"));
    println!("  End:               {}", stats.end.format("%H:%M"));
    println!("  Total Duration:    {}", format_hm(stats.duration_h));

    if stats.ramp_h > 0.0 {
        let ramp_m = (stats.ramp_h * 60.0) as i32;
        println!("  Therapy Time:      {}  (excl. {ramp_m}m ramp)", format_hm(stats.therapy_h));
        if let Some(rsp) = stats.ramp_start_pressure {
            println!("  Ramp Start Press:  {rsp:.1} cmH2O");
        }
    } else {
        println!("  Therapy Time:      {}", format_hm(stats.therapy_h));
    }

    println!();
    println!("  --- Respiratory Events ---");
    println!("  AHI:               {:.1} /hr", stats.ahi);
    println!("  Apneas:            {}  ({:.1}/hr)", stats.apnea_count, stats.apnea_index);
    println!("  Hypopneas:         {}  ({:.1}/hr)", stats.hypopnea_count, stats.hypop_index);
    if let Some(sr) = stats.snoring_ratio {
        println!("  Snoring:           {sr:.1}%");
    }
    if let Some(fl) = stats.flow_limit_ratio {
        println!("  Flow limited:      {fl:.1}%");
    }

    println!();
    println!("  --- Pressure ---");
    if stats.avg_pressure > 0.0 {
        println!("  Average:           {:.1} cmH2O", stats.avg_pressure);
    }
    if let Some(v) = stats.min_pressure {
        println!("  Min:               {v:.1} cmH2O");
    }
    if let Some(v) = stats.max_pressure {
        println!("  Max:               {v:.1} cmH2O");
    }
    if let Some(v) = stats.min_pressure_used {
        println!("  Min used (APAP):   {v:.1} cmH2O");
    }
    if let Some(v) = stats.max_pressure_used {
        println!("  Max used (APAP):   {v:.1} cmH2O");
    }

    println!();
    println!("  --- Leak ---");
    if stats.avg_leak > 0.0 {
        println!("  Average:           {:.1} L/min", stats.avg_leak);
    }
    if let Some(v) = stats.min_leak {
        println!("  Min:               {v:.1} L/min");
    }
    if let Some(v) = stats.max_leak {
        println!("  Max:               {v:.1} L/min");
    }

    println!();
    println!("  --- Session Event Log ---");
    for ev in events {
        let val_str = ev.kind.format_session_value(ev.value);
        println!("  {}  {}{val_str}", ev.datetime.format("%H:%M"), ev.kind);
    }
    println!();
}

pub fn run(dev: &mut Device, offset: usize) -> Result<()> {
    let header = dev.read_device_info()?;

    println!("Device:   {}  ({})", header.serial_number, header.model);
    println!("Events:   {} in log", header.events_in_queue);
    if header.queue_full {
        println!("Warning:  event queue full — oldest events may be overwritten");
    }
    println!();

    if header.events_in_queue == 0 {
        println!("No events recorded on device.");
        return Ok(());
    }

    let base_addr = dev.read_event_data_address()?;
    println!("Reading event log (base addr {base_addr:#06x})...");

    let events = dev.fetch_all_events(header.events_in_queue, base_addr)?;
    if events.is_empty() {
        println!("No parseable events found.");
        return Ok(());
    }
    println!("Parsed {} events from {} in queue.", events.len(), header.events_in_queue);
    println!();

    let (session_events, num, total) = find_session(&events, offset);
    if session_events.is_empty() {
        if total == 0 {
            println!("No therapy sessions found in event log.");
        } else {
            println!("Session {offset} not found. Log contains {total} session(s).");
        }
        return Ok(());
    }

    if let Some(stats) = SessionStats::from_events(session_events, header.model.is_apap()) {
        display_stats(&stats, session_events, num, total);
    }

    Ok(())
}
