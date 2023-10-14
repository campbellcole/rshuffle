# rshuffle

A dead simple MPD shuffler written in Rust. Inspired by [ashuffle](https://github.com/joshkunz/ashuffle).

This shuffler will keep track of which songs have been played and will not play them again until all songs have been played.

# Usage

```sh
rshuffle # connects to localhost on port 6600
# or
rshuffle -H <host> -p <port>
```

## Tracing

This project defaults to a `RUST_LOG` directive of `rshuffle=info`. This can be changed to e.g. `rshuffle=trace` to see more detailed logging, or to `rshuffle=off` to disable logging. (See [`EnvFilter` Directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives))

# License

This project is dual-licensed under the MIT and Apache 2.0 licenses. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
