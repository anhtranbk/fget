mod downloader;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let url = "https://pdos.csail.mit.edu/6.824/papers/mapreduce.pdf";
    downloader::download(url)
}
