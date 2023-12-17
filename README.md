# rshuffle

A dead simple MPD shuffler written in Rust. Inspired by [ashuffle](https://github.com/joshkunz/ashuffle).

This shuffler will keep track of which songs have been played and will not play them again until all songs have been played (can be disabled).
It can also keep a buffer of songs in the queue to enable crossfading (disabled by default).

# Installation

## Using Cargo

```sh
$ cargo install rshuffle
```

## From source

```sh
$ git clone https://github.com/campbellcole/rshuffle.git
$ cd rshuffle
$ cargo install --path .
```

# Usage

```sh
$ rshuffle --help
A dead simple MPD shuffler written in pure Rust.

Usage: rshuffle [OPTIONS]

Options:
  -H, --host <HOST>
          The hostname of the MPD server

          [default: 0.0.0.0]

  -p, --port <PORT>
          The port of the MPD server

          [default: 6600]

  -b, --num-buffer <NUM_BUFFER>
          The number of additional songs to keep in the playlist after the current song

          This is required for crossfade to work

          [default: 0]

  -n, --no-tracking
          Don't keep track of which songs have been played

  -f, --filter <FILTER>
          Only play songs which contain any of these strings in their titles. Can be specified multiple times

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Logging

This project defaults the `RUST_LOG` environment variable to `rshuffle=info`. This variable can be changed to e.g. `rshuffle=error` to only see errors, or to `rshuffle=off` to disable logging. (See [`EnvFilter` Directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives))

## MSRV

The minimum supported Rust version for this crate is 1.70.0.

#### Policy

**`0.x.y`:** MSRV can only change when the **minor** version is incremented (e.g. `0.1.0 -> 0.2.0`)
<br />
**`x.y.z`:** MSRV can only change when the **major** version is incremented (e.g. `1.0.0 -> 2.0.0`)

# License

This project is dual-licensed under the MIT and Apache 2.0 licenses. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
