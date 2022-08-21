use std::{error::Error, fmt};

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

pub struct Config {
    pub url: String,
    pub out_path: String,
    pub num_threads: u8,
    pub debug: bool,
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config, PError> {
        if args.len() < 2 {
            return Err(make_error("not enough arguments"));
        }

        let url = args[1].clone();
        let out_path = args.get(2).unwrap_or(&String::new()).clone();

        let mut num_threads = 4; // default value is 4
        let mut debug = false;
        if args.len() >= 3 {
            num_threads = args[3].parse::<u8>()?;
        }
        if args.len() >= 4 {
            debug = args[4].parse::<bool>()?;
        }

        Ok(Config {
            url,
            out_path,
            num_threads,
            debug,
        })
    }
}
