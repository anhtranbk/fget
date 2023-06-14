# fget

Minimal HTTP downloader in Rust.

Features:

* multiple downloads concurrently using http-range (if supported by server)
* wget style input arguments
* native support redirects
* support TLS via [native-tls](https://github.com/sfackler/rust-native-tls)
* multi progress bars (thanks to [indicatif](https://github.com/mitsuhiko/indicatif))

## How to use

```bash
./fget --help
Minimal HTTP downloader in Rust

USAGE:
    fget [OPTIONS] <URL>

ARGS:
    <URL>

OPTIONS:
    -h, --help                         Print help information
    -i, --info                         Only print response information
    -o, --output <FILE>
    -r, --no-redirect
    -t, --num-threads <NUM_THREADS>    Number of concurrent downloads (if supported by server) using
                                       http-range [default: 4]
    -T, --timeout <TIMEOUT>            TCP connection/read/write timeout in seconds [default: 10]
    -u, --user-agent <USER_AGENT>      User-Agent header to be used by the HTTP client
    -V, --version                      Print version information
```

Example:

```bash
./fget https://download.tableplus.com/macos/468/TablePlus.dmg -T10 -t4 -o/Users/anhtn/TablePlus.dmg
```