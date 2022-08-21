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
    content_type: String,
    len: u64,
    _code: u16,
}

fn format_byte_length(len: u64) -> String {
    let units = ["B", "kB", "MB", "GB", "TB"];

    let mut value = len;
    let mut unit_index = 0;

    while value >= 1024 && unit_index < units.len() - 1 {
        value /= 1024;
        unit_index += 1;
    }

    format!("{:.1} {}", value, units[unit_index])
}

fn get_download_info(client: &mut HttpClient, debug: bool) -> Result<DownloadInfo, PError> {
    let resp = client.head()?;
    println!(
        "{} {}",
        resp.status().as_u16(),
        resp.status().canonical_reason().unwrap_or_default()
    );
    if resp.status().as_u16() / 100 != 2 {
        return Err(make_error(
            format!("server response error: {}", resp.status().as_u16(),).as_str(),
        ));
    }

    let mut len = 0u64;
    let mut range_supported = false;
    let mut content_type = String::new();

    for (key, val) in resp.headers().iter() {
        match *key {
            header::CONTENT_LENGTH => len = val.to_str()?.parse::<u64>()?,
            header::ACCEPT_RANGES => {
                if val.to_str()? == "bytes" {
                    range_supported = true;
                }
            }
            header::CONTENT_TYPE => content_type = val.to_str().unwrap().to_string(),
            _ => {}
        }
    }

    if debug {
        println!("Response headers:");
        for (key, value) in resp.headers().iter() {
            println!("=> {}: {}", key, value.to_str().unwrap_or_default());
        }
        println!("");
    }

    Ok(DownloadInfo {
        range_supported,
        len,
        content_type,
        _code: resp.status().as_u16(),
    })
}

fn download<T: DownloadObserver>(
    cfg: &Config,
    url_info: &UrlInfo,
    dlinfo: &DownloadInfo,
    ob: &mut T,
) -> Result<(), PError> {
    ob.on_download_start(0, dlinfo.len);

    let resp = HttpClient::connect(&url_info)?.get()?;
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
    print!("HTTP request sent, awaiting response...");

    let dlinfo = get_download_info(&mut client, cfg.debug)?;
    println!(
        "Length: {} ({}), accept-ranges: {} [{}]",
        dlinfo.len,
        format_byte_length(dlinfo.len),
        dlinfo.range_supported,
        dlinfo.content_type
    );

    if dlinfo.len == 0 {
        return Err(make_error("content length is zero"));
    }

    // out client is one-time client, so we need to create the new one
    println!("Saving to: '{}'\r\n", url_info.fname);
    download(&cfg, &url_info, &dlinfo, ob)
}
