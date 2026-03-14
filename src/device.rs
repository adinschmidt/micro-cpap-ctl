use anyhow::{bail, Context, Result};
use chrono::{Local, TimeZone, Utc};
use std::io;
use std::thread;
use std::time::{Duration, Instant};

use crate::model::*;

const BAUD_RATE: u32 = 38400;
const CHAR_DELAY: Duration = Duration::from_millis(20);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);
/// Short read timeout so the polling loop stays responsive.
const PORT_READ_TIMEOUT: Duration = Duration::from_millis(100);

pub const MIN_PRESSURE: f64 = 4.0;
pub const MAX_PRESSURE: f64 = 20.0;

// ── Hex codec helpers (private) ──────────────────────────────────────

fn hex_int(s: &str) -> i64 {
    i64::from_str_radix(s, 16).unwrap_or(0)
}

fn swap_endian(s: &str) -> String {
    let mut pairs: Vec<&str> = Vec::new();
    let mut i = 0;
    while i + 2 <= s.len() {
        pairs.push(&s[i..i + 2]);
        i += 2;
    }
    pairs.reverse();
    pairs.join("")
}

fn hex_int_le(s: &str) -> i64 {
    hex_int(&swap_endian(s))
}

fn hex_double(s: &str, scale: f64) -> f64 {
    hex_int(s) as f64 * scale
}

fn hex_ascii(s: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i + 2 <= s.len() {
        if let Ok(code) = u8::from_str_radix(&s[i..i + 2], 16) {
            if (32..127).contains(&code) {
                out.push(code as char);
            }
        }
        i += 2;
    }
    out
}

fn encode_hex(value: i64, width: usize) -> String {
    format!("{:0>width$X}", value, width = width)
}

// ── Event parsing (pub for session/monitor modules) ──────────────────

/// Parse a 10-hex-char compliance event into a full [`Event`] with
/// UTC→local timestamp conversion.
pub fn parse_event(data: &str) -> Option<Event> {
    if data.len() < 10 || data.chars().all(|c| c == 'f' || c == 'F') {
        return None;
    }

    let date_val = hex_int(&swap_endian(&data[0..4])) as u32;
    let time_val = hex_int(&swap_endian(&data[4..8])) as u32;
    let sub_hex = &data[8..10];

    let year = ((date_val >> 9) & 0x7F) as i32 + 2000;
    let month = (date_val >> 5) & 0x0F;
    let day = date_val & 0x1F;
    let hour = (time_val >> 11) & 0x1F;
    let minute = (time_val >> 5) & 0x3F;
    let etype = (time_val & 0x1F) as u8;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 {
        return None;
    }

    let kind = EventKind::from_code(etype);
    let value = hex_int(sub_hex) as f64 * kind.scale();

    let dt_utc = Utc.with_ymd_and_hms(year, month, day, hour, minute, 0).single()?;
    let dt_local = dt_utc.with_timezone(&Local).naive_local();

    Some(Event { datetime: dt_local, kind, value })
}

/// Parse a compliance event for real-time monitoring (no date conversion).
pub fn parse_live_event(data: &str) -> Option<LiveEvent> {
    if data.len() < 10 || data.chars().all(|c| c == 'f' || c == 'F') {
        return None;
    }

    let time_val = hex_int(&swap_endian(&data[4..8])) as u32;
    let sub_hex = &data[8..10];

    let hour = (time_val >> 11) & 0x1F;
    let minute = (time_val >> 5) & 0x3F;
    let etype = (time_val & 0x1F) as u8;

    let kind = EventKind::from_code(etype);
    let value = hex_int(sub_hex) as f64 * kind.scale();

    Some(LiveEvent { hour, minute, kind, value })
}

// ── Device handle ────────────────────────────────────────────────────

pub struct Device {
    port: Box<dyn serialport::SerialPort>,
}

impl Device {
    /// Normalize the port path for the current platform.
    /// On Windows, COM ports >= 10 require the `\\.\` prefix.
    fn normalize_port(path: &str) -> String {
        #[cfg(target_os = "windows")]
        {
            if path.starts_with("COM") && !path.starts_with(r"\\.\") {
                return format!(r"\\.\{}", path);
            }
        }
        path.to_string()
    }

    /// Try to open a port and send a lightweight probe command.
    /// Returns `Ok(device_type_code)` if the device responds like a CPAP.
    pub fn probe(path: &str) -> Result<String> {
        let mut dev = Self::open(path)?;
        let resp = dev.command_timeout("Tff", Duration::from_millis(500))?;
        if resp.starts_with("Rff") {
            Ok(resp[3..7.min(resp.len())].to_string())
        } else {
            bail!("Not a CPAP device (got: {resp})")
        }
    }

    /// Scan all USB serial ports and return the first that responds to a CPAP probe.
    pub fn detect() -> Option<(String, String)> {
        let ports = serialport::available_ports().ok()?;
        for p in &ports {
            if !matches!(p.port_type, serialport::SerialPortType::UsbPort(_)) {
                continue;
            }
            if let Ok(code) = Self::probe(&p.port_name) {
                return Some((p.port_name.clone(), code));
            }
        }
        None
    }

    /// Open the serial connection to a Micro CPAP device.
    pub fn open(path: &str) -> Result<Self> {
        let path = &Self::normalize_port(path);
        let mut port = serialport::new(path, BAUD_RATE)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(PORT_READ_TIMEOUT)
            .open()
            .with_context(|| format!("Failed to open serial port {path}"))?;
        port.write_request_to_send(false)?;
        port.write_data_terminal_ready(false)?;
        Ok(Self { port })
    }

    // ── Low-level transport ──────────────────────────────────────────

    /// Send a command string and return the device response.
    /// Characters are sent one at a time (the device echoes each one),
    /// then `\r` terminates.  Response arrives after the echo, delimited
    /// by `\r`.
    fn command(&mut self, cmd: &str) -> Result<String> {
        self.command_timeout(cmd, DEFAULT_TIMEOUT)
    }

    fn command_timeout(&mut self, cmd: &str, timeout: Duration) -> Result<String> {
        self.port.clear(serialport::ClearBuffer::Input)?;

        for byte in cmd.bytes() {
            self.port.write_all(&[byte])?;
            thread::sleep(CHAR_DELAY);
        }
        self.port.write_all(b"\r")?;

        let mut buf = Vec::new();
        let deadline = Instant::now() + timeout;

        while Instant::now() < deadline {
            let mut tmp = [0u8; 512];
            match self.port.read(&mut tmp) {
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {}
                Err(e) => return Err(e.into()),
            }
            if buf.iter().filter(|&&b| b == b'\r').count() >= 2 {
                break;
            }
        }

        let text = String::from_utf8_lossy(&buf);
        let parts: Vec<&str> = text.split('\r').collect();
        Ok(if parts.len() >= 2 {
            parts[1].trim_matches('\r').to_string()
        } else {
            text.trim_matches('\r').to_string()
        })
    }

    /// Send a command, expecting a response that starts with `expect`.
    fn query(&mut self, cmd: &str, expect: &str) -> Result<String> {
        let resp = self.command(cmd)?;
        if !resp.starts_with(expect) {
            bail!("Command {cmd}: expected {expect}…, got: {resp}");
        }
        Ok(resp[expect.len()..].to_string())
    }

    fn query_timeout(&mut self, cmd: &str, expect: &str, timeout: Duration) -> Result<String> {
        let resp = self.command_timeout(cmd, timeout)?;
        if !resp.starts_with(expect) {
            bail!("Command {cmd}: expected {expect}…, got: {resp}");
        }
        Ok(resp[expect.len()..].to_string())
    }

    // ── Read commands ────────────────────────────────────────────────

    /// Read the event-log header (serial number, model, queue state).
    pub fn read_device_info(&mut self) -> Result<DeviceInfo> {
        let data = self.query("Tbd", "Rbd")?;
        if data.len() < 88 {
            bail!("Device header too short ({} chars)", data.len());
        }

        let serial_number = hex_ascii(&data[4..68]);
        let firmware_checksum = hex_ascii(&data[68..76]);
        let queue_full = hex_int(&data[2..4]) != 0;
        let events_in_queue = hex_int_le(&data[80..84]) as u32;
        let offset = hex_int_le(&data[84..88]) as u32;

        let prefix = serial_number.chars().next().unwrap_or('?');
        let model = DeviceModel::from_serial_prefix(prefix)
            .with_context(|| format!("Unknown device prefix '{prefix}'"))?;

        Ok(DeviceInfo {
            serial_number,
            model,
            firmware_checksum,
            events_in_queue,
            offset,
            queue_full,
        })
    }

    /// Read the therapy configuration.  The returned variant matches the
    /// device model (CPAP fields vs APAP fields).
    pub fn read_config(&mut self, model: DeviceModel) -> Result<TherapyConfig> {
        let data = self.query("Tab", "Rab")?;

        if model.is_apap() {
            let pressure = hex_double(&data[0..4], 0.1);
            let raw_config = data[4..19].to_string();
            let min_pressure = hex_double(&data[19..23], 0.1);
            let max_pressure = hex_double(&data[23..27], 0.1);
            let raw_reserved = data[27..32].to_string();
            let ramp_minutes = hex_int(&data[32..36]) as i32;
            let ezex_level = (hex_int(&data[36..40]) as f64 * 0.1) as i32;
            let ramp_pressure = hex_double(&data[40..44], 0.1);

            Ok(TherapyConfig::Apap {
                pressure,
                min_pressure,
                max_pressure,
                ezex_level,
                ramp: RampSettings {
                    duration_minutes: ramp_minutes,
                    start_pressure: ramp_pressure,
                },
                raw_config,
                raw_reserved,
            })
        } else {
            let pressure = hex_double(&data[0..4], 0.1);
            let raw_config = data[4..32].to_string();
            let ramp_minutes = hex_int(&data[32..36]) as i32;
            let raw_reserved = data[36..40].to_string();
            let ramp_pressure = hex_double(&data[40..44], 0.1);

            Ok(TherapyConfig::Cpap {
                pressure,
                ramp: RampSettings {
                    duration_minutes: ramp_minutes,
                    start_pressure: ramp_pressure,
                },
                raw_config,
                raw_reserved,
            })
        }
    }

    pub fn read_blower_time(&mut self) -> Result<BlowerTime> {
        let data = self.query("Tbc", "Rbc")?;
        let parts: Vec<&str> = data.split(',').collect();
        if parts.len() < 3 {
            bail!("Blower time: expected 3 fields, got {}", parts.len());
        }
        Ok(BlowerTime {
            hours: parts[0].parse()?,
            minutes: parts[1].parse()?,
            seconds: parts[2].parse()?,
        })
    }

    pub fn read_patient_hours(&mut self) -> Result<PatientHours> {
        let data = self.query("Tb8", "Rb8")?;
        let parts: Vec<&str> = data.split(',').collect();
        if parts.len() < 6 {
            bail!("Patient hours: expected 6 fields, got {}", parts.len());
        }
        Ok(PatientHours {
            therapy_hours: parts[0].parse()?,
            therapy_minutes: parts[1].parse()?,
            therapy_seconds: parts[2].parse()?,
            sessions_over_8h: parts[3].parse::<f64>()? as i32,
            sessions_6_to_8h: parts[4].parse::<f64>()? as i32,
            sessions_4_to_6h: parts[5].parse::<f64>()? as i32,
        })
    }

    pub fn read_pressure_goal(&mut self) -> Result<f64> {
        let data = self.query("Ta1", "R41")?;
        Ok(data.trim_matches(',').parse()?)
    }

    pub fn read_device_type_code(&mut self) -> Result<String> {
        let data = self.query("Tff", "Rff")?;
        Ok(data[..4.min(data.len())].to_string())
    }

    pub fn read_calibration_offset(&mut self) -> Result<f64> {
        let data = self.query("Tb3", "Rb3")?;
        if data.len() < 2 {
            bail!("Calibration data too short");
        }
        let sign = &data[0..1];
        let offset = hex_double(&data[1..2], 0.1);
        Ok(if sign == "1" { -offset } else { offset })
    }

    // ── Live monitor readings ────────────────────────────────────────

    pub fn read_monitor(&mut self) -> Result<MonitorReading> {
        let data = self.query("Ta3", "Ra3")?;
        let parts: Vec<&str> = data.split(',').collect();
        if parts.len() < 5 {
            bail!("Monitor data: expected 5 fields, got {}", parts.len());
        }
        Ok(MonitorReading {
            pressure_goal: hex_double(parts[0], 0.1),
            measured_pressure: hex_double(parts[1], 0.1),
            lung_flow: hex_double(parts[2], 0.1),
            leak: hex_double(parts[3], 0.1),
            mode: DeviceMode::from_code(hex_int(parts[4]) as u8),
        })
    }

    pub fn read_flow(&mut self) -> Result<FlowReading> {
        let data = self.query("Tc3", "Rc3")?;
        let parts: Vec<&str> = data.split(',').collect();
        if parts.len() < 2 {
            bail!("Flow data: expected 2 fields, got {}", parts.len());
        }
        Ok(FlowReading {
            hose_flow: parts[0].parse()?,
            baseline_flow: parts[1].parse()?,
        })
    }

    pub fn read_pressure_sensor(&mut self) -> Result<f64> {
        let data = self.query("T60", "R60")?;
        if data.len() < 4 {
            bail!("Pressure sensor data too short");
        }
        Ok(hex_double(&data[..4], 0.1))
    }

    // ── Event log access ─────────────────────────────────────────────

    pub fn read_event_data_address(&mut self) -> Result<u32> {
        let data = self.query("Ta8", "Ra8")?;
        Ok(hex_int(&data[..4.min(data.len())]) as u32)
    }

    /// Read a raw block of compliance data from the device flash.
    pub fn read_block(&mut self, addr: u32, num_bytes: u32) -> Result<String> {
        let cmd = format!("Ta9{:04X}{:04X}", addr, num_bytes);
        let timeout = Duration::from_secs_f64(2.0_f64.max(2.0 + num_bytes as f64 * 2.0 / 3840.0));
        self.query_timeout(&cmd, "Ra9", timeout)
    }

    /// Read all events from the device event log.
    /// Skips the first 50 bytes (preamble), then reads in 1000-byte chunks.
    pub fn fetch_all_events(&mut self, events_in_queue: u32, base_addr: u32) -> Result<Vec<Event>> {
        const SKIP: u32 = 50;
        const CHUNK: u32 = 1000;

        // Skip preamble.
        let _ = self.read_block(base_addr, SKIP);

        let mut events = Vec::new();
        let mut addr = base_addr + SKIP;
        let mut remaining = events_in_queue;

        while remaining > 0 {
            let batch = CHUNK.min(remaining * 5);
            let raw = match self.read_block(addr, batch) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("  Warning: block read failed at {addr:#x}: {e}");
                    break;
                }
            };

            let mut parsed = 0u32;
            let mut i = 0;
            while i + 10 <= raw.len() {
                if let Some(ev) = parse_event(&raw[i..i + 10]) {
                    events.push(ev);
                    parsed += 1;
                }
                i += 10;
            }

            if parsed < batch / 5 {
                break;
            }
            addr += batch;
            remaining -= batch / 5;
        }

        events.truncate(events_in_queue as usize);
        Ok(events)
    }

    /// Fetch newly-appended live events starting at `addr`.
    pub fn fetch_live_events(&mut self, addr: u32, count: u32) -> Result<(Vec<LiveEvent>, u32)> {
        let num_bytes = count * 5;
        let raw = self.read_block(addr, num_bytes)?;
        let mut events = Vec::new();
        let mut i = 0;
        while i + 10 <= raw.len() {
            if let Some(ev) = parse_live_event(&raw[i..i + 10]) {
                events.push(ev);
            }
            i += 10;
        }
        Ok((events, addr + num_bytes))
    }

    // ── Write commands ───────────────────────────────────────────────

    pub fn write_cpap_config(
        &mut self,
        pressure: f64,
        raw_config: &str,
        ramp_minutes: i32,
        raw_reserved: &str,
        ramp_pressure: f64,
    ) -> Result<bool> {
        let cmd = format!(
            "Tac{}{}{}{}{}",
            encode_hex((pressure * 10.0) as i64, 4),
            raw_config,
            encode_hex(ramp_minutes as i64, 4),
            raw_reserved,
            encode_hex((ramp_pressure * 10.0) as i64, 4),
        );
        let resp = self.command(&cmd)?;
        Ok(resp.starts_with("R55"))
    }

    pub fn write_apap_config(
        &mut self,
        pressure: f64,
        raw_config: &str,
        min_pressure: f64,
        max_pressure: f64,
        raw_reserved: &str,
        ramp_minutes: i32,
        ezex: i32,
        ramp_pressure: f64,
    ) -> Result<bool> {
        let cmd = format!(
            "Tcc{}{}{}{}{}{}{}{}",
            encode_hex((pressure * 10.0) as i64, 4),
            raw_config,
            encode_hex((min_pressure * 10.0) as i64, 4),
            encode_hex((max_pressure * 10.0) as i64, 4),
            raw_reserved,
            encode_hex(ramp_minutes as i64, 4),
            encode_hex((ezex * 10) as i64, 4),
            encode_hex((ramp_pressure * 10.0) as i64, 4),
        );
        let resp = self.command(&cmd)?;
        Ok(resp.starts_with("R55"))
    }
}
