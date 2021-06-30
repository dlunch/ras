# Rust Airplay Server

Only unencrypted RAOP service (Airport Express) is implemented now.

## Local testing

Install [pyatv](https://github.com/postlund/pyatv) and execute: `atvremote -n test stream_file=<filename>`

# Log

Print all logs without raw mdns packet
`RUST_LOG="ras,ras::mdns=debug"`

# References

- https://openairplay.github.io/airplay-spec/introduction.html
- https://emanuelecozzi.net/docs/airplay2/protocols/
- https://github.com/postlund/pyatv
- https://github.com/mikebrady/shairport-sync
