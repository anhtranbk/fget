# fget

Minimal HTTP downloader in Rust.

```bash
./fget --help
fget 0.1.0

USAGE:
    fget [OPTIONS] <URL>

ARGS:
    <URL>

OPTIONS:
    -h, --help                         Print help information
    -i, --info                         Only print response information
    -o, --output <FILE>
    -r, --no-redirect
    -t, --num-threads <NUM_THREADS>    [default: 4]
    -T, --timeout <TIMEOUT>            TCP connection/read/write timeout in seconds [default: 10]
    -V, --version                      Print version information
```

Example:

```bash
./fget https://download.tableplus.com/macos/468/TablePlus.dmg -T10 -t4 -o/Users/anhtn/TablePlus.dmg
```