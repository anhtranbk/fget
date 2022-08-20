use std::env;

use fget::Config;

mod downloader;
mod http;
mod pb;

fn main() {
    let args = env::args().collect::<Vec<String>>();
    let cfg = Config::build(&args).unwrap_or_else(|err| {
        panic!("Problem parsing arguments: {err}");
    });

    let mut pbm = pb::ProgressManager::new(cfg.num_threads as usize);
    if let Err(e) = downloader::run(&cfg, &mut pbm) {
        eprintln!("An error occurred: {}", e)
    }
}
