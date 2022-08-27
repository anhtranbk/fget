use crate::{
    httpx::{resolve_addr, HttpClient, UrlInfo},
    Config,
};
use fget::{make_error, map, PError, VoidResult};
use http::header;

use std::{
    cmp,
    fs::{self, File},
    io::{BufWriter, Read, Write},
    str,
    sync::mpsc::{self, Sender},
    thread,
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

#[derive(Debug)]
enum DownloadStatus {
    Started(u8, u64, u64),
    Progress(u8, u64),
    Failed(u8, String),
    Done(u8, String),
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

fn download_part(
    url_info: &UrlInfo,
    start: u64,
    end: u64,
    idx: u8,
    sender: &Sender<DownloadStatus>,
) -> Result<(), PError> {
    let headers = map!(
        header::RANGE.to_string() => format!("bytes={}-{}", start, end)
    );
    let resp = HttpClient::connect(&url_info)?.get_with_headers(&headers)?;

    if resp.status().as_u16() / 100 != 2 {
        return Err(make_error(
            format!("server response error: {}", resp.status().as_u16(),).as_str(),
        ));
    } else {
        sender.send(DownloadStatus::Started(idx, start, end))?;
    }

    let mut r = resp.into_body();
    let mut buf = [0u8; 8192];
    let mut pos = start;

    let dir = std::env::temp_dir();
    let fpath = format!(
        "{}/{}.{}",
        dir.to_str().unwrap_or("/tmp"),
        url_info.fname,
        idx
    );
    let mut file = File::create(&fpath)?;

    while pos < end {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }

        // take a slice of buffer from 0 to nth-offset to ensure we only write newly bytes to file
        file.write_all(&buf[..n])?;
        pos += n as u64;
        sender.send(DownloadStatus::Progress(idx, pos))?;
    }

    sender.send(DownloadStatus::Done(idx, fpath))?;

    Ok(())
}

fn merge_parts(fpath: &String, parts: &Vec<String>) -> VoidResult {
    let tmp_path = format!("{}.tmp", fpath);
    let mut w = BufWriter::new(File::create(&tmp_path)?);
    let mut buf = [0u8; 8192];

    for part in parts {
        println!("Mergeing part: {:?}", part);
        let mut r = File::open(part)?;
        let n = r.read(&mut buf)?;
        if n > 0 {
            w.write_all(&buf[..n])?;
        } else {
            println!("Finished reading part {:?}", part);
            continue;
        }
    }

    w.flush()?;
    drop(w); // drop the file to close it before renaming

    println!("Rename temp file from {} to {}", tmp_path, fpath);
    fs::rename(&tmp_path, &fpath)?;

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

    let num_threads = if dlinfo.range_supported {
        cfg.num_threads as u64
    } else {
        1
    };
    let chunk_size = (dlinfo.len + num_threads - 1) / num_threads;

    let (sender, recv) = mpsc::channel();
    let mut handles = vec![];
    let mut dlparts = vec![String::default(); num_threads as usize];

    for i in 0..num_threads {
        let start = i * chunk_size;
        let end = cmp::min((i + 1) * chunk_size - 1, dlinfo.len - 1);

        let _sender = sender.clone();
        let _url_info = url_info.clone();
        let idx = i as u8;
        let handle = thread::spawn(move || {
            if let Err(err) = download_part(&_url_info, start, end, idx, &_sender) {
                _sender
                    .send(DownloadStatus::Failed(idx, err.to_string()))
                    .unwrap();
            }
        });

        handles.push(handle);
    }

    for msg in recv {
        match msg {
            DownloadStatus::Started(idx, start, end) => ob.on_download_start(idx, end - start),
            DownloadStatus::Progress(idx, pos) => ob.on_progress(idx, pos),
            DownloadStatus::Failed(idx, err) => {
                ob.on_download_end(idx);
                return Err(make_error(
                    format!("download failed at part {}: {}", idx, err).as_str(),
                ));
            }
            DownloadStatus::Done(idx, fpath) => {
                dlparts[idx as usize] = fpath;
                ob.on_download_end(idx);
            }
        }
    }

    merge_parts(&out_path, &dlparts)?;

    for handle in handles {
        handle.join().unwrap();
    }

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

    let client = HttpClient::connect(&url_info)?;
    println!("connected.");
    print!("HTTP request sent, awaiting response... ");

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
