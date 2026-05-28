<h1 align="center"><code>easytab</code></h1>

[![Crates.io](https://img.shields.io/crates/v/easytab.svg)](https://crates.io/crates/easytab)
[![Docs.rs](https://docs.rs/easytab/badge.svg)](https://docs.rs/easytab)

`easytab` is a Rust crate built for vendor-agnostic interaction with pen & tablet devices. It aims to have a very simple interface that can be easily integrated with other code.

```toml
easytab = "0.1.0"
```

## Features

- Windows support.
- [`raw-window-handle`](https://crates.io/crates/raw-window-handle) support with the `raw-window-handle` feature.

## Example

See `examples/basic.rs` for an example with `winit`.

## Roadmap

- [x] Implement basic Windows support.
- [ ] Implement basic Linux support.
- [ ] Implement basic MacOS support.
- [ ] Implement support for selecting specific tablet devices.
  - *I have this functionality mostly implemented but I'm yet to decide if it is: A) useful, and B) within scope of this crate.*
