use crate::{
    httpx::{resolve_addr, HttpClient, UrlInfo},
    Config,
};
use fget::{make_error, map, PError};
use http::header;

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    str,
    sync::{Arc, Mutex},
};

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

fn get_download_info(client: HttpClient, debug: bool) -> Result<DownloadInfo, PError> {
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

fn write_to_file(file: &Arc<Mutex<File>>, buf: &[u8], offset: u64) -> Result<(), PError> {
    let lock = file.lock();
    match lock {
        Ok(mut _file) => {
            // for readers:
            // don't need to pay attention to this comment, this is just
            // a note to help me understand Rust ownership system
            // we borrow Arc variable as immutable but Arc is owner of the File inside it
            // so when dereferencing File from Arc we can call seek and write_all methods
            // (which require a mutable reference) without error
            _file.seek(SeekFrom::Start(offset))?;
            _file.write_all(&buf)?;

            Ok(())
        }
        Err(_) => Err(make_error("could not get file lock")),
    }
}

fn download_part<T: DownloadObserver>(
    url_info: &UrlInfo,
    start: u64,
    end: u64,
    file: Arc<Mutex<File>>, // moved here
    ob: &mut T,
    idx: u8,
) -> Result<(), PError> {
    let len = end - start;
    ob.on_download_start(idx, len);

    let headers = map!(header::RANGE.to_string() => format!("bytes={}-{}", start, end));
    let resp = HttpClient::connect(&url_info)?.get_with_headers(&headers)?;
    let mut r = resp.into_body();

    let mut buf = [0u8; 8192];
    let mut pos = start;

    while pos < end {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }

        // take a slice of buffer from 0 to nth-offset to ensure we only write newly bytes to file
        write_to_file(&file, &buf[..n], pos)?;
        pos += n as u64;
        ob.on_progress(idx, pos);
    }

    ob.on_download_end(idx);
    Ok(())
}

fn download<T: DownloadObserver>(
    cfg: &Config,
    url_info: &UrlInfo,
    dlinfo: &DownloadInfo,
    ob: &mut T,
) -> Result<(), PError> {
    let mut out_path = &url_info.fname;
    if cfg.out_path.len() > 0 {
        out_path = &cfg.out_path;
    }
    let file = Arc::new(Mutex::new(File::create(out_path)?));

    download_part(url_info, 0, dlinfo.len, file, ob, 0)
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

    let client = HttpClient::connect(&url_info)?;
    println!("connected.");
    print!("HTTP request sent, awaiting response...");

    // our http client is one-time client, so we must move it
    // to let get_download_info use it instead of borrow
    let dlinfo = get_download_info(client, cfg.debug)?;
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

    println!("Saving to: '{}'\r\n", url_info.fname);
    download(&cfg, &url_info, &dlinfo, ob)
}
