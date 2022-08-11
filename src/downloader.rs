use crate::{
    http::{resolve_addr, HttpClient, UrlInfo},
    Config
};
use fget::{make_error, PError};

use std::str;

pub trait DownloadObserver {
    fn on_download_start(&mut self, part: u8, total_size: u64);
    fn on_progress(&mut self, part: u8, progress: u64);
    fn on_download_end(&mut self, part: u8);

    fn on_message(&mut self, msg: &str);
}

const BUFFER_SIZE: usize = 8192;

struct DownloadInfo(u64, bool);

fn get_download_info(client: &HttpClient, url_info: &UrlInfo) -> Result<DownloadInfo, PError> {
    let resp = client.get(url_info.path.as_str())?;

    let mut len = 0u64;
    let mut range_supported = false;

    let headers = resp.headers();
    for (key, val) in headers.iter() {
        let val = val.to_str()?;
        match key.as_str() {
            "Content-Length" => len = val.parse::<u64>()?,
            "Accept-Ranges" => range_supported = val == "bytes",
            _ => continue,
        }
    }

    Ok(DownloadInfo(len, range_supported))
}

fn download(
    client: &HttpClient,
    url_info: &UrlInfo,
    out_path: &str,
    range_supported: bool,
) -> Result<(), PError> {
    let mut buf = [0u8; BUFFER_SIZE];

    // stream.write_all(req.as_bytes())?;
    // stream.read(&mut buf)?;

    let s = str::from_utf8(&buf)?;
    for line in s.lines() {
        println!("{}", line.trim());
    }

    Ok(())
}

pub fn run<T: DownloadObserver>(cfg: &Config, _: &mut T) -> Result<(), PError> {
    println!("Downloading file at {}...", cfg.url);
    let url_info = UrlInfo::parse(&cfg.url)?;

    print!("Resolving {}... ", url_info.domain);
    let sock_addr = resolve_addr(&url_info.host_addr())?;
    println!("{}", sock_addr.ip());

    print!(
        "Connecting to ({})|{}:{}... ",
        url_info.domain,
        sock_addr.ip(),
        url_info.port
    );
    let client = HttpClient::connect(&url_info)?;
    println!("connected.");

    let dlinfo = get_download_info(&client, &url_info)?;
    let DownloadInfo(total_size, range_supported) = dlinfo;
    if total_size == 0 {
        return Err(make_error("content length is zero"));
    }

    download(&client, &url_info, &cfg.out_path, range_supported)
}
