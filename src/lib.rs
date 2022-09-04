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

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Config {
    pub url: String,

    #[clap(short, long, value_parser, value_name = "file")]
    pub output: Option<String>,

    #[clap(
        short,
        long,
        value_parser,
        default_value_t = 4,
        value_name = "num_threads"
    )]
    pub num_threads: u8,

    #[clap(short, long, value_parser, default_value_t = false)]
    pub info: bool,
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
