use fget::Config;

mod downloader;
mod httpx;
mod pb;
mod urlinfo;

fn main() {
    let cfg = Config::build().unwrap_or_else(|err| {
        panic!("Problem parsing arguments: {err}");
    });

    let mut pbm = pb::ProgressManager::new();
    if let Err(e) = downloader::run(&cfg, &mut pbm) {
        eprintln!("An error occurred: {}", e)
    }
}
