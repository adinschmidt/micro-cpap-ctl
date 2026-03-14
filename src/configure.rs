use anyhow::{bail, Result};
use std::io::{self, Write};

use crate::device::{Device, MAX_PRESSURE, MIN_PRESSURE};
use crate::info;
use crate::model::TherapyConfig;

pub struct SetArgs {
    pub pressure: Option<f64>,
    pub ramp_pressure: Option<f64>,
    pub ramp_time: Option<i32>,
    pub min_pressure: Option<f64>,
    pub max_pressure: Option<f64>,
    pub ezex: Option<i32>,
    pub yes: bool,
}

/// Round to one decimal place.
fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

pub fn run(dev: &mut Device, args: SetArgs) -> Result<()> {
    let (_, config) = info::display(dev)?;

    // ── Resolve new values from current config + overrides ───────────

    let mut new_pressure = args.pressure.unwrap_or(config.pressure());
    let ramp = config.ramp();
    let mut new_ramp_pressure = ramp.start_pressure;
    let mut new_ramp_minutes = ramp.duration_minutes;

    match (args.ramp_time, args.ramp_pressure) {
        (Some(0), _) => {
            new_ramp_minutes = 0;
            new_ramp_pressure = MIN_PRESSURE;
        }
        (Some(t), rp) => {
            new_ramp_minutes = t;
            if let Some(rp) = rp {
                new_ramp_pressure = rp;
            } else if new_ramp_minutes >= 5 && new_ramp_pressure < MIN_PRESSURE {
                new_ramp_pressure = MIN_PRESSURE;
            }
        }
        (None, Some(rp)) => {
            new_ramp_pressure = rp;
            if new_ramp_minutes < 5 {
                new_ramp_minutes = 5;
            }
        }
        (None, None) => {}
    }

    // Clamp / validate.
    new_pressure = round1(new_pressure.clamp(MIN_PRESSURE, MAX_PRESSURE));
    new_ramp_pressure = round1(new_ramp_pressure.clamp(MIN_PRESSURE, new_pressure - 1.0));
    if new_ramp_minutes != 0 {
        new_ramp_minutes = new_ramp_minutes.clamp(5, 45);
    }
    if new_pressure < MIN_PRESSURE + 1.0 {
        new_ramp_minutes = 0;
        new_ramp_pressure = MIN_PRESSURE;
    }

    // ── Collect changes ──────────────────────────────────────────────

    let mut changes: Vec<String> = Vec::new();
    let eps = 0.05;

    if (new_pressure - config.pressure()).abs() > eps {
        changes.push(format!(
            "  Therapy Pressure:    {:.1} -> {:.1} cmH2O",
            config.pressure(),
            new_pressure
        ));
    }
    if new_ramp_minutes != ramp.duration_minutes {
        changes.push(format!(
            "  Ramp Duration:       {} -> {} min",
            ramp.duration_minutes, new_ramp_minutes
        ));
    }
    if (new_ramp_pressure - ramp.start_pressure).abs() > eps {
        changes.push(format!(
            "  Ramp Pressure:       {:.1} -> {:.1} cmH2O",
            ramp.start_pressure, new_ramp_pressure
        ));
    }

    // APAP-specific fields.
    let (new_min, new_max, new_ezex) = if let TherapyConfig::Apap {
        min_pressure,
        max_pressure,
        ezex_level,
        ..
    } = &config
    {
        let mut nm = args.min_pressure.unwrap_or(*min_pressure);
        let mut nx = args.max_pressure.unwrap_or(*max_pressure);
        let mut ne = args.ezex.unwrap_or(*ezex_level);

        nm = round1(nm.clamp(MIN_PRESSURE, new_pressure));
        nx = round1(nx.clamp(new_pressure, MAX_PRESSURE));
        ne = ne.clamp(0, 3);

        if (nm - min_pressure).abs() > eps {
            changes.push(format!("  Min Pressure:        {min_pressure:.1} -> {nm:.1} cmH2O"));
        }
        if (nx - max_pressure).abs() > eps {
            changes.push(format!("  Max Pressure:        {max_pressure:.1} -> {nx:.1} cmH2O"));
        }
        if ne != *ezex_level {
            changes.push(format!("  EZEX Level:          {ezex_level} -> {ne}"));
        }

        (nm, nx, ne)
    } else {
        (0.0, 0.0, 0)
    };

    if changes.is_empty() {
        println!();
        println!("=== Planned Changes ===");
        println!("  No changes needed.");
        return Ok(());
    }

    println!();
    println!("=== Planned Changes ===");
    for c in &changes {
        println!("{c}");
    }

    // ── Confirmation ─────────────────────────────────────────────────

    if !args.yes {
        println!();
        print!("Apply these changes? [y/N] ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if answer.trim().to_lowercase() != "y" {
            println!("Aborted.");
            return Ok(());
        }
    }

    // ── Write ────────────────────────────────────────────────────────

    println!();
    println!("Writing configuration...");

    let ok = match &config {
        TherapyConfig::Cpap {
            raw_config,
            raw_reserved,
            ..
        } => dev.write_cpap_config(
            new_pressure,
            raw_config,
            new_ramp_minutes,
            raw_reserved,
            new_ramp_pressure,
        )?,
        TherapyConfig::Apap {
            raw_config,
            raw_reserved,
            ..
        } => dev.write_apap_config(
            new_pressure,
            raw_config,
            new_min,
            new_max,
            raw_reserved,
            new_ramp_minutes,
            new_ezex,
            new_ramp_pressure,
        )?,
    };

    if ok {
        println!("  Success!");
        println!();
        println!("Verifying...");
        info::display(dev)?;
    } else {
        bail!("Failed to write configuration");
    }

    Ok(())
}
