use crate::{make_error, Config, PError};
use native_tls::{TlsConnector, TlsStream};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    str,
};

pub trait DownloadObserver {
    fn on_download_start(&mut self, part: u8, total_size: u64);
    fn on_progress(&mut self, part: u8, progress: u64);
    fn on_download_end(&mut self, part: u8);

    fn on_message(&mut self, msg: &str);
}

const BUFFER_SIZE: usize = 8192;
const DEFAULT_HEADERS: [&'static str; 4] = [
    "User-Agent: fget/0.1.0",
    "Accept: */*",
    "Accept-Encoding: identity",
    "Connection: Keep-Alive",
];
const HTTP_HEAD: &str = "HEAD";
const HTTP_GET: &str = "GET";

struct UrlInfo {
    domain: String,
    addr: SocketAddr,
    download_path: String,
    fname: String,
    https: bool,
}

struct DownloadInfo(u64, bool);

trait ReadWrite: Read + Write {}

impl<T: Read + Write> ReadWrite for T {}

fn resolve_url(url: &str) -> Result<UrlInfo, PError> {
    let parts: Vec<&str> = url.split("/").collect();
    let domain = parts[2];
    let port = if parts[0].starts_with("http") {
        80
    } else {
        443
    };

    let addr = format!("{}:{}", domain, port);
    print!("Resolving {}... ", domain);

    let sock_addrs = addr.to_socket_addrs()?;
    if sock_addrs.len() == 0 {
        return Err(make_error("invalid host address"));
    }

    let mut sock_addr = sock_addrs.next().unwrap();
    let sock_addr = sock_addrs
        .filter(|ip| ip.is_ipv4())
        .next()
        .unwrap_or_else(|| sock_addr); // try to use ipv4 address if available
    println!("{}", sock_addr.ip());

    let query_idx = parts[0].len() + parts[1].len() + 2;
    Ok(UrlInfo {
        domain: domain.to_string(),
        addr: sock_addr,
        download_path: url[query_idx..].to_string(),
        fname: parts[parts.len() - 1 as usize].to_string(),
        https: port == 443,
    })
}

fn parse_header(header: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = header.split(":").collect();
    if parts.len() != 2 {
        return None;
    }

    Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
}

fn make_http_request(method: &str, url_info: &UrlInfo) -> String {
    format!("{} {} HTTP/1.1\r\n", method, url_info.download_path)
        + format!("Host: {}\r\n", url_info.domain).as_str()
        + DEFAULT_HEADERS.join("\r\n").as_str()
        + "\r\n"
}

fn get_download_info<T: ReadWrite>(
    stream: &mut T,
    url_info: &UrlInfo,
) -> Result<DownloadInfo, PError> {
    let req = make_http_request(HTTP_HEAD, &url_info);
    stream.write_all(req.as_bytes())?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;

    let mut total_size = 0u64;
    let mut range_supported = false;

    for line in buf.lines() {
        if let Some(header) = parse_header(line) {
            let (key, val) = header;
            match key.as_str() {
                "Content-Length" => total_size = val.parse::<u64>()?,
                "Accept-Ranges" => range_supported = val.as_str() == "bytes",
                _ => continue,
            }
        }
    }

    Ok(DownloadInfo(total_size, range_supported))
}

fn download<T: ReadWrite>(
    stream: &mut T,
    url_info: &UrlInfo,
    opath: &str,
    range_supported: bool,
) -> Result<(), PError> {
    let mut buf = [0u8; BUFFER_SIZE];
    let req = format!("GET {} HTTP/1.1\r\n", url_info.download_path)
        + format!("Host: {}\r\n", url_info.domain).as_str()
        + DEFAULT_HEADERS.join("\r\n").as_str()
        + "\r\n";
    println!("req body: {:?}\n", req);

    stream.write_all(req.as_bytes())?;
    stream.read(&mut buf)?;

    let s = str::from_utf8(&buf)?;
    for line in s.lines() {
        println!("{}", line.trim());
    }

    while let Some(nread) = stream.read(&mut buf).ok() {
        if nread > 0 {
            println!("{} bytes read", nread);
        } else {
            break;
        }
    }

    Ok(())
}

fn open_conn(url_info: &UrlInfo) -> Result<Box<dyn ReadWrite>, PError> {
    let stream = TcpStream::connect(&url_info.addr)?;
    println!("connected.");

    if url_info.https {
        let tls_conn = TlsConnector::new()?;
        let stream = tls_conn.connect(url_info.domain.as_str(), stream)?;
        Ok(Box::new(stream))
    } else {
        Ok(Box::new(stream))
    }
}

pub fn run<T: DownloadObserver>(cfg: &Config, ob: &mut T) -> Result<(), PError> {
    println!("Downloading file at {}...", cfg.url);
    let url_info = resolve_url(&cfg.url)?;

    print!("Connecting to {}|{}... ", url_info.domain, url_info.addr);

    let mut stream = open_conn(&url_info)?;
    println!("connected.");

    let dlinfo = get_download_info(&mut stream, &url_info)?;
    let DownloadInfo(total_size, range_supported) = dlinfo;
    if total_size == 0 {
        return Err(make_error("content length is zero"));
    }

    download(&mut stream, &url_info, &cfg.out_path, range_supported)
}
