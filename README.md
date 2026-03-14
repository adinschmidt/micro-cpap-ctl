# micro-cpap-ctl

> **Disclaimer:** This project is not affiliated with, endorsed by, or associated with Somnetics or any CPAP device manufacturer. It is an independently developed, open-source tool that communicates with Micro CPAP devices over their serial interface through protocol analysis. This software is provided "as is" without warranty of any kind. **This software is not a medical device and should not be relied upon for medical treatment decisions.** Modifying therapy settings without guidance from a qualified sleep medicine provider may be harmful. The author assumes no liability for device damage, data loss, or health consequences resulting from use of this software. Use at your own risk.

---

A command-line tool for monitoring and configuring Micro CPAP devices over serial.

## Features

- **`info`** — Read device identity, therapy configuration, and usage statistics
- **`monitor`** — Live real-time therapy data with pressure, flow, and leak graphs
- **`set`** — Modify therapy settings (pressure, ramp, APAP min/max, EZEX level)
- **`session`** — View detailed session statistics (AHI, pressure, leak, event log)
- **`list-ports`** — List available serial ports to find your device

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
micro-cpap-ctl list-ports

# Read device info
micro-cpap-ctl info

# Live monitor (default 0.5s polling)
micro-cpap-ctl monitor

# View most recent session
micro-cpap-ctl session

# View 3rd most recent session
micro-cpap-ctl session -n 3

# Change therapy pressure
micro-cpap-ctl set --pressure 12.0

# Change pressure with auto-confirm
micro-cpap-ctl set --pressure 12.0 --ramp-time 20 --ramp-pressure 6.0 -y

# APAP: set min/max pressure range
micro-cpap-ctl set --min-pressure 8.0 --max-pressure 16.0
```

## Serial Connection

Communicates at **38400 baud** (8N1, no flow control). Connect via USB-to-serial adapter to the device's serial port.

The port is **auto-detected** — the tool scans for USB serial devices on all platforms. If no USB device is found, it falls back to:
- **macOS:** `/dev/tty.usbserial-*`, `/dev/tty.usbmodem-*`
- **Linux:** `/dev/ttyUSB0`
- **Windows:** `COM3`

Override with `--port <device>`. Run `list-ports` to see available devices.

> **Windows note:** COM ports 10+ are handled automatically (the `\\.\` prefix is added internally).

## License

[MIT](LICENSE)
