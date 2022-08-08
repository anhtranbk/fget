mod chatgpt;
mod downloader;
mod pb;

fn main() {
    let url = std::env::args().nth(1).expect("url must be provided");
    let opath = std::env::args().nth(2).expect("opath must be provided");
    // let num_threads = std::env::args()
    //     .nth(3)
    //     .unwrap_or_else(|| String::from("4")) // default value is 4
    //     .parse::<u8>()
    //     .unwrap();

    let pbm = pb::ProgressManager::new();
    let res = downloader::run(url.as_str(), opath.as_str(), pbm);

    // let res = chatgpt::download(&url, &opath, num_threads);
    match res {
        Ok(_) => println!("Download complete!"),
        Err(e) => eprintln!("Error downloading file: {}", e),
    }
}
