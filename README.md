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

## Design Decisions

### Sequence flag (SQ) is an encoding detail

The IEC 60870-5 wire format supports two layouts for information objects within an ASDU:
- **SQ=0**: each object carries its own Information Object Address (IOA)
- **SQ=1**: only the first object has an IOA; subsequent IOAs are implied as sequential (base + 1, base + 2, ...)

In lib60870-rs, SQ is treated as a **wire-format detail transparent to the user**. The `Asdu` struct always stores explicit `(IOA, object)` pairs regardless of SQ mode. During decoding, SQ=1 frames have their sequential addresses computed automatically. During encoding, if `is_sequence` is set, the encoder validates that IOAs are consecutive and emits the compact SQ=1 format.

## License

MIT
