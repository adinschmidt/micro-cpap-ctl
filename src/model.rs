use chrono::NaiveDateTime;
use std::fmt;

// ── Device identity ──────────────────────────────────────────────────

/// Device model, determined by the first character of the serial number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceModel {
    StandardCpap, // 'A'
    AutoPap,      // 'B'
    CpapWithEzex, // 'C'
}

impl DeviceModel {
    pub fn from_serial_prefix(c: char) -> Option<Self> {
        match c {
            'A' => Some(Self::StandardCpap),
            'B' => Some(Self::AutoPap),
            'C' => Some(Self::CpapWithEzex),
            _ => None,
        }
    }

    pub fn is_apap(self) -> bool {
        matches!(self, Self::AutoPap | Self::CpapWithEzex)
    }
}

impl fmt::Display for DeviceModel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::StandardCpap => write!(f, "Standard CPAP"),
            Self::AutoPap => write!(f, "AutoPAP"),
            Self::CpapWithEzex => write!(f, "CPAP with EZEX"),
        }
    }
}

/// Aggregate device identity read from the event-log header.
#[derive(Debug)]
#[allow(dead_code)]
pub struct DeviceInfo {
    pub serial_number: String,
    pub model: DeviceModel,
    pub firmware_checksum: String,
    pub events_in_queue: u32,
    pub offset: u32,
    pub queue_full: bool,
}

// ── Operating mode ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum DeviceMode {
    Standby,
    Therapy,
    Calibration,
    Unknown(u8),
}

impl DeviceMode {
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Standby,
            1 | 2 => Self::Therapy,
            3 => Self::Calibration,
            n => Self::Unknown(n),
        }
    }
}

impl fmt::Display for DeviceMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Standby => write!(f, "Standby"),
            Self::Therapy => write!(f, "Therapy"),
            Self::Calibration => write!(f, "Calibration"),
            Self::Unknown(n) => write!(f, "Mode {n}"),
        }
    }
}

// ── Therapy configuration ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RampSettings {
    pub duration_minutes: i32,
    pub start_pressure: f64,
}

/// Configuration stored on the device.  Variant encodes the device type
/// so callers never need to guess which fields exist.
#[derive(Debug, Clone)]
pub enum TherapyConfig {
    Cpap {
        pressure: f64,
        ramp: RampSettings,
        /// Opaque config bytes the device expects back when writing.
        raw_config: String,
        raw_reserved: String,
    },
    Apap {
        pressure: f64,
        min_pressure: f64,
        max_pressure: f64,
        ezex_level: i32,
        ramp: RampSettings,
        raw_config: String,
        raw_reserved: String,
    },
}

impl TherapyConfig {
    pub fn pressure(&self) -> f64 {
        match self {
            Self::Cpap { pressure, .. } | Self::Apap { pressure, .. } => *pressure,
        }
    }

    pub fn ramp(&self) -> &RampSettings {
        match self {
            Self::Cpap { ramp, .. } | Self::Apap { ramp, .. } => ramp,
        }
    }
}

// ── Usage statistics ─────────────────────────────────────────────────

#[derive(Debug)]
pub struct BlowerTime {
    pub hours: f64,
    pub minutes: f64,
    pub seconds: f64,
}

impl BlowerTime {
    pub fn total_hours(&self) -> f64 {
        self.hours + self.minutes / 60.0 + self.seconds / 3600.0
    }
}

impl fmt::Display for BlowerTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}h {}m {}s ({:.1}h)",
            self.hours as i32,
            self.minutes as i32,
            self.seconds as i32,
            self.total_hours()
        )
    }
}

#[derive(Debug)]
pub struct PatientHours {
    pub therapy_hours: f64,
    pub therapy_minutes: f64,
    pub therapy_seconds: f64,
    pub sessions_over_8h: i32,
    pub sessions_6_to_8h: i32,
    pub sessions_4_to_6h: i32,
}

impl PatientHours {
    pub fn total_hours(&self) -> f64 {
        self.therapy_hours + self.therapy_minutes / 60.0 + self.therapy_seconds / 3600.0
    }
}

impl fmt::Display for PatientHours {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}h {}m {}s ({:.1}h)",
            self.therapy_hours as i32,
            self.therapy_minutes as i32,
            self.therapy_seconds as i32,
            self.total_hours()
        )
    }
}

// ── Live monitor readings ────────────────────────────────────────────

#[derive(Debug)]
pub struct MonitorReading {
    pub pressure_goal: f64,
    pub measured_pressure: f64,
    pub lung_flow: f64,
    pub leak: f64,
    pub mode: DeviceMode,
}

#[derive(Debug)]
pub struct FlowReading {
    pub hose_flow: f64,
    pub baseline_flow: f64,
}

// ── Compliance events ────────────────────────────────────────────────

/// Strongly typed event kinds.  Every protocol event code maps to exactly
/// one variant, and all the "what does this event mean?" logic lives here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    SessionStart,
    SessionEnd,
    RampStart,
    RampEnd,
    LeakReport,
    SupplyVoltage,
    Apnea,
    Hypopnea,
    PressureReduced,
    PressureAverage,
    MinPressureSetting,
    MaxPressureSetting,
    EzexLevel,
    MinPressureUsed,
    MaxPressureUsed,
    FlowLimitedRatio,
    SnoringRatio,
    MinLeak,
    MaxLeak,
    AverageLeak,
    PressureIncreasedApnea,
    PressureIncreasedHypopnea,
    PressureIncreasedCombination,
    PressureIncreasedSnoring,
    PressureIncreasedFlowLimit,
    PressureIncreasedCommand,
    Unknown(u8),
}

impl EventKind {
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::SessionStart,
            2 => Self::SessionEnd,
            5 => Self::RampStart,
            6 => Self::RampEnd,
            7 => Self::LeakReport,
            8 => Self::SupplyVoltage,
            9 => Self::Apnea,
            10 => Self::Hypopnea,
            11 => Self::PressureReduced,
            12 => Self::PressureAverage,
            13 => Self::MinPressureSetting,
            14 => Self::MaxPressureSetting,
            15 => Self::EzexLevel,
            16 => Self::MinPressureUsed,
            17 => Self::MaxPressureUsed,
            18 => Self::FlowLimitedRatio,
            19 => Self::SnoringRatio,
            20 => Self::MinLeak,
            21 => Self::MaxLeak,
            22 => Self::AverageLeak,
            23 => Self::PressureIncreasedApnea,
            24 => Self::PressureIncreasedHypopnea,
            25 => Self::PressureIncreasedCombination,
            26 => Self::PressureIncreasedSnoring,
            27 => Self::PressureIncreasedFlowLimit,
            28 => Self::PressureIncreasedCommand,
            n => Self::Unknown(n),
        }
    }

    /// Scale factor applied to the raw sub-data byte.
    pub fn scale(self) -> f64 {
        match self {
            Self::SessionStart
            | Self::PressureReduced
            | Self::PressureAverage
            | Self::MinPressureSetting
            | Self::MaxPressureSetting
            | Self::EzexLevel
            | Self::MinPressureUsed
            | Self::MaxPressureUsed
            | Self::FlowLimitedRatio
            | Self::SnoringRatio
            | Self::PressureIncreasedApnea
            | Self::PressureIncreasedHypopnea
            | Self::PressureIncreasedCombination
            | Self::PressureIncreasedSnoring
            | Self::PressureIncreasedFlowLimit
            | Self::PressureIncreasedCommand => 0.1,
            _ => 1.0,
        }
    }

    /// Whether this event represents a change in therapy pressure.
    pub fn is_pressure_change(self) -> bool {
        matches!(
            self,
            Self::PressureReduced
                | Self::PressureAverage
                | Self::PressureIncreasedApnea
                | Self::PressureIncreasedHypopnea
                | Self::PressureIncreasedCombination
                | Self::PressureIncreasedSnoring
                | Self::PressureIncreasedFlowLimit
                | Self::PressureIncreasedCommand
        )
    }

    /// ANSI colour code for live-monitor display.
    pub fn ansi_color(self) -> &'static str {
        match self {
            Self::Apnea | Self::PressureIncreasedApnea | Self::PressureIncreasedCombination => {
                "\x1b[1;31m" // red
            }
            Self::Hypopnea
            | Self::PressureIncreasedHypopnea
            | Self::PressureIncreasedSnoring
            | Self::PressureIncreasedFlowLimit => "\x1b[1;33m", // yellow
            Self::PressureIncreasedCommand | Self::SessionStart | Self::SessionEnd => {
                "\x1b[1;36m" // cyan
            }
            Self::PressureReduced => "\x1b[1;32m", // green
            _ => "\x1b[0m",
        }
    }

    /// Human-readable value suffix for the live monitor event log.
    pub fn format_monitor_value(self, value: f64) -> String {
        if value == 0.0 {
            return String::new();
        }
        match self {
            Self::Apnea | Self::Hypopnea => format!("  ({value:.0}s)"),
            Self::PressureReduced
            | Self::PressureIncreasedApnea
            | Self::PressureIncreasedHypopnea
            | Self::PressureIncreasedCombination
            | Self::PressureIncreasedSnoring
            | Self::PressureIncreasedFlowLimit
            | Self::PressureIncreasedCommand => format!("  -> {value:.1} cmH2O"),
            Self::FlowLimitedRatio | Self::SnoringRatio => format!("  {value:.1}%"),
            Self::PressureAverage | Self::MinPressureUsed | Self::MaxPressureUsed => {
                format!("  {value:.1} cmH2O")
            }
            _ => String::new(),
        }
    }

    /// Human-readable value suffix for the session event log.
    pub fn format_session_value(self, value: f64) -> String {
        match self {
            Self::Apnea | Self::Hypopnea => format!("  ({value:.0}s)"),
            Self::PressureReduced
            | Self::PressureAverage
            | Self::MinPressureUsed
            | Self::MaxPressureUsed
            | Self::PressureIncreasedApnea
            | Self::PressureIncreasedHypopnea
            | Self::PressureIncreasedCombination
            | Self::PressureIncreasedSnoring
            | Self::PressureIncreasedFlowLimit
            | Self::PressureIncreasedCommand => format!("  {value:.1} cmH2O"),
            Self::FlowLimitedRatio | Self::SnoringRatio => format!("  {value:.1}%"),
            Self::LeakReport | Self::MinLeak | Self::MaxLeak | Self::AverageLeak => {
                format!("  {value:.1} L/min")
            }
            Self::SessionStart if value > 0.0 => format!("  {value:.1} cmH2O"),
            Self::RampStart => format!("  ({:.1} cmH2O)", value * 0.1),
            _ if value != 0.0 => format!("  {value}"),
            _ => String::new(),
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Self::SessionStart => "Session start",
            Self::SessionEnd => "Session end",
            Self::RampStart => "Ramp start",
            Self::RampEnd => "Ramp end",
            Self::LeakReport => "Leak report",
            Self::SupplyVoltage => "Supply voltage",
            Self::Apnea => "Apnea",
            Self::Hypopnea => "Hypopnea",
            Self::PressureReduced => "Pressure reduced",
            Self::PressureAverage => "Pressure average",
            Self::MinPressureSetting => "Min pressure setting",
            Self::MaxPressureSetting => "Max pressure setting",
            Self::EzexLevel => "EZEX level",
            Self::MinPressureUsed => "Min pressure used",
            Self::MaxPressureUsed => "Max pressure used",
            Self::FlowLimitedRatio => "Flow limited ratio",
            Self::SnoringRatio => "Snoring ratio",
            Self::MinLeak => "Min leak",
            Self::MaxLeak => "Max leak",
            Self::AverageLeak => "Average leak",
            Self::PressureIncreasedApnea => "Pressure increased (apnea)",
            Self::PressureIncreasedHypopnea => "Pressure increased (hypopnea)",
            Self::PressureIncreasedCombination => "Pressure increased (combination)",
            Self::PressureIncreasedSnoring => "Pressure increased (snoring)",
            Self::PressureIncreasedFlowLimit => "Pressure increased (flow limit)",
            Self::PressureIncreasedCommand => "Pressure increased (command)",
            Self::Unknown(n) => return write!(f, "Event {n}"),
        };
        write!(f, "{name}")
    }
}

// ── Parsed event structs ─────────────────────────────────────────────

/// Full event with local-time timestamp (for session analysis).
#[derive(Debug, Clone)]
pub struct Event {
    pub datetime: NaiveDateTime,
    pub kind: EventKind,
    pub value: f64,
}

/// Lightweight event for real-time monitoring (no date conversion).
#[derive(Debug, Clone)]
pub struct LiveEvent {
    pub hour: u32,
    pub minute: u32,
    pub kind: EventKind,
    pub value: f64,
}

impl fmt::Display for LiveEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let val = self.kind.format_monitor_value(self.value);
        write!(
            f,
            "  {}{:02}:{:02}  {}{}\x1b[0m",
            self.kind.ansi_color(),
            self.hour,
            self.minute,
            self.kind,
            val
        )
    }
}

// ── Session statistics ───────────────────────────────────────────────

/// Computed statistics for a single therapy session.
#[derive(Debug)]
pub struct SessionStats {
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub in_progress: bool,
    pub duration_h: f64,
    pub therapy_h: f64,
    pub ramp_h: f64,
    pub ramp_start_pressure: Option<f64>,
    pub apnea_count: u32,
    pub hypopnea_count: u32,
    pub apnea_index: f64,
    pub hypop_index: f64,
    pub ahi: f64,
    pub avg_pressure: f64,
    pub min_pressure: Option<f64>,
    pub max_pressure: Option<f64>,
    pub min_pressure_used: Option<f64>,
    pub max_pressure_used: Option<f64>,
    pub avg_leak: f64,
    pub min_leak: Option<f64>,
    pub max_leak: Option<f64>,
    pub snoring_ratio: Option<f64>,
    pub flow_limit_ratio: Option<f64>,
}

impl SessionStats {
    /// Compute statistics from a slice of session events.
    pub fn from_events(events: &[Event], is_apap: bool) -> Option<Self> {
        if events.is_empty() {
            return None;
        }

        let first = &events[0];
        let last = &events[events.len() - 1];
        let in_progress = last.kind != EventKind::SessionEnd;
        let start_dt = first.datetime;
        let end_dt = last.datetime;
        let duration_h = (end_dt - start_dt).num_seconds() as f64 / 3600.0;

        let mut apnea_count = 0u32;
        let mut hypopnea_count = 0u32;
        let mut cur_pressure = first.value;
        let mut cur_pressure_t = start_dt;
        let mut pressure_weighted = 0.0f64;
        let mut pressure_minutes = 0.0f64;
        let mut min_pressure = if cur_pressure > 0.0 { Some(cur_pressure) } else { None };
        let mut max_pressure = min_pressure;
        let mut cur_leak = 0.0f64;
        let mut cur_leak_t = start_dt;
        let mut leak_weighted = 0.0f64;
        let mut min_leak: Option<f64> = None;
        let mut max_leak: Option<f64> = None;
        let mut ramp_start_t: Option<NaiveDateTime> = None;
        let mut ramp_total_h = 0.0f64;
        let mut ramp_start_pressure: Option<f64> = None;
        let mut snoring_ratio: Option<f64> = None;
        let mut flow_limit_ratio: Option<f64> = None;
        let mut min_pressure_used: Option<f64> = None;
        let mut max_pressure_used: Option<f64> = None;

        let flush_pressure = |t: NaiveDateTime, p: f64, pt: &mut NaiveDateTime, pw: &mut f64, pm: &mut f64| {
            if p > 0.0 {
                let mins = (t - *pt).num_seconds() as f64 / 60.0;
                *pw += p * mins;
                *pm += mins;
            }
            *pt = t;
        };

        let flush_leak = |t: NaiveDateTime, l: f64, lt: &mut NaiveDateTime, lw: &mut f64| {
            let mins = (t - *lt).num_seconds() as f64 / 60.0;
            *lw += l * mins;
            *lt = t;
        };

        for ev in &events[1..] {
            let t = ev.datetime;
            let val = ev.value;

            match ev.kind {
                EventKind::SessionEnd => {
                    flush_pressure(t, cur_pressure, &mut cur_pressure_t, &mut pressure_weighted, &mut pressure_minutes);
                    flush_leak(t, cur_leak, &mut cur_leak_t, &mut leak_weighted);
                }
                EventKind::Apnea => apnea_count += 1,
                EventKind::Hypopnea => hypopnea_count += 1,
                kind if kind.is_pressure_change() => {
                    flush_pressure(t, cur_pressure, &mut cur_pressure_t, &mut pressure_weighted, &mut pressure_minutes);
                    cur_pressure = val;
                    if val > 0.0 {
                        min_pressure = Some(min_pressure.map_or(val, |m: f64| m.min(val)));
                        max_pressure = Some(max_pressure.map_or(val, |m: f64| m.max(val)));
                    }
                }
                EventKind::LeakReport if !is_apap => {
                    flush_leak(t, cur_leak, &mut cur_leak_t, &mut leak_weighted);
                    cur_leak = val;
                    min_leak = Some(min_leak.map_or(val, |m: f64| m.min(val)));
                    max_leak = Some(max_leak.map_or(val, |m: f64| m.max(val)));
                }
                EventKind::AverageLeak if is_apap => {
                    flush_leak(t, cur_leak, &mut cur_leak_t, &mut leak_weighted);
                    cur_leak = val;
                    min_leak = Some(min_leak.map_or(val, |m: f64| m.min(val)));
                    max_leak = Some(max_leak.map_or(val, |m: f64| m.max(val)));
                }
                EventKind::RampStart => {
                    ramp_start_t = Some(t);
                    ramp_start_pressure = Some(val * 0.1);
                }
                EventKind::RampEnd => {
                    if let Some(rst) = ramp_start_t {
                        ramp_total_h += (t - rst).num_seconds() as f64 / 3600.0;
                        ramp_start_t = None;
                    }
                }
                EventKind::MinPressureUsed => min_pressure_used = Some(val),
                EventKind::MaxPressureUsed => max_pressure_used = Some(val),
                EventKind::FlowLimitedRatio => flow_limit_ratio = Some(val),
                EventKind::SnoringRatio => snoring_ratio = Some(val),
                _ => {}
            }
        }

        let therapy_h = (duration_h - ramp_total_h).max(0.0);
        let avg_pressure = if pressure_minutes > 0.0 {
            pressure_weighted / pressure_minutes
        } else {
            cur_pressure
        };
        let avg_leak = if therapy_h > 0.0 {
            leak_weighted / (therapy_h * 60.0)
        } else {
            0.0
        };
        let ai = if therapy_h > 0.0 { apnea_count as f64 / therapy_h } else { 0.0 };
        let hi = if therapy_h > 0.0 { hypopnea_count as f64 / therapy_h } else { 0.0 };

        Some(Self {
            start: start_dt,
            end: end_dt,
            in_progress,
            duration_h,
            therapy_h,
            ramp_h: ramp_total_h,
            ramp_start_pressure,
            apnea_count,
            hypopnea_count,
            apnea_index: ai,
            hypop_index: hi,
            ahi: ai + hi,
            avg_pressure,
            min_pressure,
            max_pressure,
            min_pressure_used,
            max_pressure_used,
            avg_leak,
            min_leak,
            max_leak,
            snoring_ratio,
            flow_limit_ratio,
        })
    }
}

/// Format hours as "Xh YYm".
pub fn format_hm(hours: f64) -> String {
    let h = hours as i32;
    let m = ((hours - h as f64) * 60.0) as i32;
    format!("{h}h {m:02}m")
}
