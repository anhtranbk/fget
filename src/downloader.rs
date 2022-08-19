use crate::{
    http::{resolve_addr, HttpClient, UrlInfo},
    Config,
};
use fget::{make_error, PError};
use http::header;

use std::str;

pub trait DownloadObserver {
    fn on_download_start(&mut self, part: u8, total_size: u64);
    fn on_progress(&mut self, part: u8, progress: u64);
    fn on_download_end(&mut self, part: u8);

    fn on_message(&mut self, msg: &str);
}

const BUFFER_SIZE: usize = 8192;

struct DownloadInfo(u64, bool);

fn get_download_info(client: &mut HttpClient, url_info: &UrlInfo) -> Result<DownloadInfo, PError> {
    let resp = client.head(url_info.path.as_str())?;

    let mut len = 0u64;
    let mut range_supported = false;

    let headers = resp.headers();
    if let Some(val) = headers.get(header::CONTENT_LANGUAGE) {
        len = val.to_str()?.parse::<u64>()?;
    }
    if let Some(val) = headers.get(header::ACCEPT_RANGES) {
        if val.to_str()? == "bytes" {
            range_supported = true;
        }
    }

    // only for debugging purposes
    for (key, value) in headers.iter() {
        println!("header => {}:{}", key, value.to_str().unwrap_or_default());
    }

    Ok(DownloadInfo(len, range_supported))
}

fn download(
    _client: &HttpClient,
    _url_info: &UrlInfo,
    _out_path: &str,
    _range_supported: bool,
) -> Result<(), PError> {
    panic!("download fn is not implemented");
}

pub fn run<T: DownloadObserver>(cfg: &Config, _: &mut T) -> Result<(), PError> {
    println!("Downloading file at {}...", cfg.url);
    let url_info = UrlInfo::parse(&cfg.url)?;
    println!("resolved url {:?}", url_info);

    print!("Resolving {}... ", url_info.domain);
    let sock_addr = resolve_addr(&url_info.host_addr())?;
    println!("{}", sock_addr.ip());

    print!(
        "Connecting to ({})|{}:{}... ",
        url_info.domain,
        sock_addr.ip(),
        url_info.port
    );
    let mut client = HttpClient::connect(&url_info)?;
    println!("connected.");
    println!("HTTP request sent, awaiting response...");

    let dlinfo = get_download_info(&mut client, &url_info)?;
    let DownloadInfo(total_size, range_supported) = dlinfo;
    if total_size == 0 {
        return Err(make_error("content length is zero"));
    }

    download(&client, &url_info, &cfg.out_path, range_supported)
}
