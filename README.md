# lib60870-rs

Rust implementation of the IEC 60870-5-101 and IEC 60870-5-104 protocols for SCADA and power system communication.

## Features

- IEC 60870-5-104 (TCP/IP) client and server
- IEC 60870-5-101 (serial) master and slave
- All standard ASDU type IDs and information objects
- Async I/O with tokio
- Optional TLS support (IEC 62351)

## Usage

```toml
[dependencies]
lib60870 = "0.1"
```

Enable TLS:

```toml
[dependencies]
lib60870 = { version = "0.1", features = ["tls"] }
```

## License

MIT
