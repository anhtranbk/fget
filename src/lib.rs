use std::{error::Error, fmt};

use clap::Parser;

#[allow(dead_code)]
#[macro_export]
macro_rules! map {
    ($($key:expr => $value:expr),*) => {
        {
            let mut map = std::collections::HashMap::new();
            $(map.insert($key, $value);)*
            map
        }
    };
}

#[allow(dead_code)]
#[macro_export]
macro_rules! hash_map {
    {$($k: expr => $v: expr),* $(,)?} => {
        std::collections::HashMap::from([$(($k, $v),)*])
    };
}

#[allow(dead_code)]
#[macro_export]
macro_rules! hash_map_e {
    {$($k: expr => $v: expr),* $(,)?} => {
        std::collections::HashMap::from([$(($k, $v as _),)*])
    };
}

/// box of error (pointer to actual error object)
pub type PError = Box<dyn Error>;
pub type VoidResult = Result<(), PError>;

#[derive(Debug, Clone)]
struct FgetError(String);

impl fmt::Display for FgetError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for FgetError {}

pub fn make_error(err: &str) -> PError {
    Box::new(FgetError(err.to_string()))
}

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Config {
    pub url: String,

    #[clap(short, long, value_parser, value_name = "FILE")]
    pub output: Option<String>,

    #[clap(
        short,
        long,
        value_parser,
        help = "User-Agent header to be used by the HTTP client"
    )]
    pub user_agent: Option<String>,

    #[clap(
        short = 't',
        long,
        value_parser,
        default_value_t = 4,
        help = "Number of concurrent downloads (if supported by server) using http-range"
    )]
    pub num_threads: u8,

    #[clap(
        short,
        long,
        value_parser,
        action,
        help = "Only print response information"
    )]
    pub info: bool,

    // no-redirect seems like a pointless option but I add this option just to examine
    // how to use Rust enum via RedirectPolicy, haha =))
    #[clap(short = 'r', long, value_parser, action)]
    pub no_redirect: bool,

    #[clap(
        short = 'T',
        long,
        value_parser,
        default_value_t = 10,
        help = "TCP connection/read/write timeout in seconds"
    )]
    pub timeout: u8,
}

impl Config {
    pub fn build() -> Result<Config, PError> {
        let cfg = Config::parse();
        if cfg.num_threads <= 0 || cfg.num_threads > 32 {
            return Err(make_error(
                "invalid number of threads, must be between 1 and 32",
            ));
        }

        Ok(cfg)
    }
}
