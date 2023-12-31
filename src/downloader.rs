use crate::{
    httpx::{resolve_addr, HttpClient, HttpResponse, RedirectPolicy},
    urlinfo::UrlInfo,
    Config,
};
use fget::{make_error, map, PError};
use http::header;

use std::{
    cmp,
    fs::{self, File},
    io::{BufWriter, Read, Write},
    sync::mpsc::{self, Sender},
    thread,
};

pub trait DownloadObserver {
    fn on_init(&mut self, len: usize);
    fn on_download_start(&mut self, idx: u8, len: u64);
    fn on_progress(&mut self, idx: u8, pos: u64);
    fn on_download_end(&mut self, idx: u8);
}

struct DownloadInfo {
    range_supported: bool,
    content_type: String,
    len: u64,
}

#[derive(Debug)]
enum DownloadStatus {
    Started(u8, u64),
    Progress(u8, u64),
    Failed(u8, String),
    Done(u8, String),
}

/// format byte length in bytes to human readable
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

fn build_client(cfg: &Config, urlinfo: &UrlInfo) -> Result<HttpClient, PError> {
    let mut builder = HttpClient::builder().from_url_info(&urlinfo);
    if cfg.no_redirect {
        builder = builder.with_redirect_policy(RedirectPolicy::None);
    }
    if cfg.timeout > 0 {
        builder = builder.with_timeout_ms(cfg.timeout as u64 * 1000);
    }
    if let Some(ua) = &cfg.user_agent {
        builder = builder.with_user_agent(ua);
    }

    builder.build()
}

fn get_download_info(resp: HttpResponse) -> Result<DownloadInfo, PError> {
    let mut len = 0u64;
    let mut range_supported = false;
    let mut content_type = String::new();

    for (key, val) in resp.headers().iter() {
        let val = val.to_str()?;
        match *key {
            header::CONTENT_LENGTH => len = val.parse::<u64>()?,
            header::ACCEPT_RANGES => range_supported = val == "bytes",
            header::CONTENT_TYPE => content_type = val.to_string(),
            _ => {}
        }
    }

    Ok(DownloadInfo {
        range_supported,
        len,
        content_type,
    })
}

fn download_part(
    cfg: &Config,
    urlinfo: &UrlInfo,
    start: u64,
    end: u64,
    idx: u8,
    sender: &Sender<DownloadStatus>,
) -> Result<(), PError> {
    let headers = map!(
        header::RANGE.to_string() => format!("bytes={}-{}", start, end)
    );
    let client = build_client(cfg, urlinfo)?;
    let resp = client.get_with_headers(&urlinfo.path, &headers)?;

    let mut r = resp.into_body();
    let mut buf = [0u8; 8192];
    let mut pos = start;

    let dir = std::env::temp_dir();
    let fpath = format!(
        "{}{}.{}",
        dir.to_str().unwrap_or("/tmp/"),
        urlinfo.fname,
        idx
    );
    let mut file = File::create(&fpath)?;

    // start fetching data file from server
    sender.send(DownloadStatus::Started(idx, end - start))?;

    while pos < end {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }

        // take a slice of buffer from 0 to nth-offset to ensure we only write newly bytes to file
        file.write_all(&buf[..n])?;
        pos += n as u64;
        sender.send(DownloadStatus::Progress(idx, pos - start))?;
    }

    sender.send(DownloadStatus::Done(idx, fpath))?;

    Ok(())
}

fn merge_parts(fpath: &String, parts: &Vec<String>) -> Result<(), PError> {
    // if there is only one part, just rename downloaded file
    if parts.len() == 1 {
        fs::rename(parts.get(0).unwrap(), &fpath)?;
        return Ok(());
    }

    let tmp_path = format!("{}.tmp", fpath);
    let mut w = BufWriter::new(File::create(&tmp_path)?);
    let mut buf = [0u8; 8192];

    for part in parts {
        let mut r = File::open(part)?;
        loop {
            let n = r.read(&mut buf)?;
            if n > 0 {
                w.write_all(&buf[..n])?;
            } else {
                break;
            }
        }
    }

    w.flush()?;
    drop(w); // drop the file to close it before renaming

    fs::rename(&tmp_path, &fpath)?;

    Ok(())
}

fn download<T: DownloadObserver>(
    cfg: &Config,
    urlinfo: &UrlInfo,
    dlinfo: &DownloadInfo,
    ob: &mut T,
) -> Result<(), PError> {
    let num_threads = if dlinfo.range_supported {
        cfg.num_threads as u64
    } else {
        1
    };
    let chunk_size = (dlinfo.len + num_threads - 1) / num_threads;

    // update UI (progress bar) before starting downloads
    ob.on_init(num_threads as usize);

    let (sender, recv) = mpsc::channel();
    let mut handles = vec![];

    for i in 0..num_threads {
        let start = i * chunk_size;
        let end = cmp::min((i + 1) * chunk_size - 1, dlinfo.len - 1);

        // below seems stupid but with my current knowledge about Rust, using clone is the
        // easiest way to share object between multi-thread, even though I know that
        // url_info and cfg are read-only objects and can be safe to read by multiple threads
        let _sender = sender.clone();
        let _urlinfo = urlinfo.clone();
        let _idx = i as u8;
        let _cfg = cfg.clone();

        let handle = thread::spawn(move || {
            if let Err(err) = download_part(&_cfg, &_urlinfo, start, end, _idx, &_sender) {
                _sender
                    .send(DownloadStatus::Failed(_idx, err.to_string()))
                    .unwrap(); // TODO: find a safe way to handle this
            }
        });

        handles.push(handle);
    }

    let mut cnt = num_threads; // number of remaining downloads
    let mut dlparts = vec![String::default(); num_threads as usize];

    // block until all download threads are done or an error is encountered
    for msg in recv {
        match msg {
            DownloadStatus::Started(idx, len) => ob.on_download_start(idx, len),
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

                cnt -= 1;
                if cnt == 0 {
                    break;
                }
            }
        }
    }

    for handle in handles {
        // TODO: is there a better way ?
        handle.join().unwrap();
    }

    // merge all download parts into one file
    let output = cfg.output.as_ref().unwrap_or(&urlinfo.fname);
    merge_parts(&output, &dlparts)?;
    println!(
        "File downloaded to '{}': {} ({})",
        output,
        dlinfo.len,
        format_byte_length(dlinfo.len)
    );

    Ok(())
}

pub fn run<T: DownloadObserver>(cfg: &Config, ob: &mut T) -> Result<(), PError> {
    println!("Downloading file at {}", cfg.url);
    let urlinfo = UrlInfo::parse(&cfg.url)?;

    print!("Resolving {}... ", urlinfo.domain);
    let sock_addr = resolve_addr(&urlinfo.host_addr())?;
    println!("{}", sock_addr.ip());

    print!(
        "Connecting to ({})|{}:{}... ",
        urlinfo.domain,
        sock_addr.ip(),
        urlinfo.port
    );

    let client = build_client(cfg, &urlinfo)?;
    println!("connected.");
    println!("HTTP request sent, awaiting response... ");

    // our http client is one-time client, so we must move it
    // to let get_download_info use it instead of borrow
    let resp = client.head(&urlinfo.path)?;
    println!(
        "{} {}",
        resp.status().as_u16(),
        resp.status().canonical_reason().unwrap_or_default()
    );

    if cfg.info {
        println!("Response headers:");
        for (key, value) in resp.headers().iter() {
            println!("=> {}: {}", key, value.to_str().unwrap_or_default());
        }

        return Ok(());
    }

    let dlinfo = get_download_info(resp)?;
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

    println!("Saving to: '{}'\r\n", urlinfo.fname);
    download(&cfg, &urlinfo, &dlinfo, ob)
}
