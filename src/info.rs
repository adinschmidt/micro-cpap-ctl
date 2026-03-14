use anyhow::Result;

use crate::device::Device;
use crate::model::{DeviceModel, TherapyConfig};

/// Display device info + config and return the model & config for
/// callers that need to modify settings afterwards.
pub fn display(dev: &mut Device) -> Result<(DeviceModel, TherapyConfig)> {
    // ── Identity ─────────────────────────────────────────────────────
    let info = dev.read_device_info()?;
    println!();
    println!("=== Device Info ===");
    println!("  Serial Number:     {}", info.serial_number);
    println!("  Device Type:       {}", info.model);
    println!("  Firmware Checksum: {}", info.firmware_checksum);
    println!("  Events in Queue:   {}", info.events_in_queue);

    if let Ok(code) = dev.read_device_type_code() {
        println!("  Device Type Code:  {code}");
    }

    // ── Configuration ────────────────────────────────────────────────
    let config = dev.read_config(info.model)?;
    println!();
    println!("=== Configuration ===");

    match &config {
        TherapyConfig::Cpap { pressure, ramp, .. } => {
            println!("  Therapy Pressure:      {pressure:.1} cmH2O");
            println!("  Ramp Duration:         {} minutes", ramp.duration_minutes);
            println!("  Ramp Start Pressure:   {:.1} cmH2O", ramp.start_pressure);
        }
        TherapyConfig::Apap {
            pressure,
            min_pressure,
            max_pressure,
            ezex_level,
            ramp,
            ..
        } => {
            println!("  Therapy Pressure:      {pressure:.1} cmH2O");
            println!("  Min Therapy Pressure:  {min_pressure:.1} cmH2O");
            println!("  Max Therapy Pressure:  {max_pressure:.1} cmH2O");
            println!("  EZEX Level:            {ezex_level}");
            println!("  Ramp Duration:         {} minutes", ramp.duration_minutes);
            println!("  Ramp Start Pressure:   {:.1} cmH2O", ramp.start_pressure);
        }
    }

    if let Ok(pg) = dev.read_pressure_goal() {
        println!("  Pressure Goal:         {pg}");
    }

    // ── Usage stats ──────────────────────────────────────────────────
    println!();
    println!("=== Usage Stats ===");

    if let Ok(bt) = dev.read_blower_time() {
        println!("  Blower Time:       {bt}");
    }

    if let Ok(ph) = dev.read_patient_hours() {
        println!("  Therapy Time:      {ph}");
        println!("  Sessions >8h:      {}", ph.sessions_over_8h);
        println!("  Sessions 6-8h:     {}", ph.sessions_6_to_8h);
        println!("  Sessions 4-6h:     {}", ph.sessions_4_to_6h);
    }

    if let Ok(cal) = dev.read_calibration_offset() {
        println!("  Calibration:       {cal:+.1} cmH2O");
    }

    Ok((info.model, config))
}

pub fn run(dev: &mut Device) -> Result<()> {
    display(dev)?;
    Ok(())
}
