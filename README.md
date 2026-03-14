# cpap

> **Disclaimer:** This project is not affiliated with, endorsed by, or associated with Somnetics or any CPAP device manufacturer. It is an independently developed, open-source tool that communicates with Micro CPAP devices over their serial interface through protocol analysis. This software is provided "as is" without warranty of any kind. **This software is not a medical device and should not be relied upon for medical treatment decisions.** Modifying therapy settings without guidance from a qualified sleep medicine provider may be harmful. The author assumes no liability for device damage, data loss, or health consequences resulting from use of this software. Use at your own risk.

---

A command-line tool for monitoring and configuring Micro CPAP devices over serial.

## Features

- **`info`** — Read device identity, therapy configuration, and usage statistics
- **`monitor`** — Live real-time therapy data with pressure, flow, and leak graphs
- **`set`** — Modify therapy settings (pressure, ramp, APAP min/max, EZEX level)
- **`session`** — View detailed session statistics (AHI, pressure, leak, event log)
- **`detect`** — Scan USB serial ports to find a connected CPAP device
- **`list-ports`** — List all available serial ports

Supports Standard CPAP, AutoPAP, and CPAP with EZEX device variants.

## Installation

```sh
cargo install --path .
```

Or build from source:

```sh
cargo build --release
```

## Usage

```sh
# Find your serial port
cpap list-ports

# Read device info
cpap info

# Live monitor (default 0.5s polling)
cpap monitor

# View most recent session
cpap session

# View 3rd most recent session
cpap session -n 3

# Change therapy pressure
cpap set --pressure 12.0

# Change pressure with auto-confirm
cpap set --pressure 12.0 --ramp-time 20 --ramp-pressure 6.0 -y

# APAP: set min/max pressure range
cpap set --min-pressure 8.0 --max-pressure 16.0
```

## Serial Connection

Communicates at **38400 baud** (8N1, no flow control). Connect via USB-to-serial adapter to the device's serial port.

The device is **auto-detected** — when no `--port` is specified, the tool probes each USB serial port with a lightweight query to find a device that actually responds. This works across macOS, Linux, and Windows.

```sh
# Explicitly scan for the device
cpap detect

# Or just run any command — it will auto-detect
cpap info
```

Override with `--port <device>` if you have multiple serial devices or want to skip the scan.

> **Windows note:** COM ports 10+ are handled automatically (the `\\.\` prefix is added internally).

## License

[MIT](LICENSE)
