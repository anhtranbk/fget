use crate::{
    http::{resolve_addr, HttpClient, UrlInfo},
    Config,
};
use fget::{make_error, PError};
use http::header;

use std::{io::Read, str};

pub trait DownloadObserver {
    fn on_download_start(&mut self, idx: u8, len: u64);
    fn on_progress(&mut self, idx: u8, pos: u64);
    fn on_download_end(&mut self, idx: u8);

    fn on_message(&mut self, msg: &str);
}

struct DownloadInfo {
    range_supported: bool,
    len: u64,
}

fn get_download_info(client: &mut HttpClient, url_info: &UrlInfo) -> Result<DownloadInfo, PError> {
    let resp = client.head(url_info.path.as_str())?;

    let mut len = 0u64;
    let mut range_supported = false;

    let headers = resp.headers();
    if let Some(val) = headers.get(header::CONTENT_LENGTH) {
        len = val.to_str()?.parse::<u64>()?;
        println!("=> found content-len: {:?}", val);
    }
    if let Some(val) = headers.get(header::ACCEPT_RANGES) {
        if val.to_str()? == "bytes" {
            range_supported = true;
        }
        println!("=> found acccept-range: {:?}", val);
    }

    // only for debugging purposes
    for (key, value) in headers.iter() {
        println!("header => {}: {}", key, value.to_str().unwrap_or_default());
    }

    Ok(DownloadInfo {
        range_supported,
        len,
    })
}

fn download<T: DownloadObserver>(
    client: &mut HttpClient,
    cfg: &Config,
    url_info: &UrlInfo,
    dlinfo: &DownloadInfo,
    ob: &mut T,
) -> Result<(), PError> {
    ob.on_download_start(0, dlinfo.len);

    let resp = client.get(&url_info.path)?;
    let mut r = resp.into_body();
    let mut buf = [0u8; 8192];
    let mut pr = 0u64;

    while pr < dlinfo.len {
        let n = r.read(&mut buf)?;
        pr += n as u64;
        ob.on_progress(0, pr)
    }

    ob.on_download_end(0);

    Ok(())
}

pub fn run<T: DownloadObserver>(cfg: &Config, ob: &mut T) -> Result<(), PError> {
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
    let mut client = HttpClient::connect(&url_info)?;
    println!("connected.");
    println!("HTTP request sent, awaiting response...");

    let dlinfo = get_download_info(&mut client, &url_info)?;
    if dlinfo.len == 0 {
        return Err(make_error("content length is zero"));
    }

    // out client is one-time client, so we need to create the new one
    let mut client = HttpClient::connect(&url_info)?;
    download(&mut client, &cfg, &url_info, &dlinfo, ob)
}
