use native_tls::{TlsConnector, TlsStream};
use std::{
    error::Error,
    fmt,
    io::{Read, Write},
    net::TcpStream,
    str,
};

const BUFFER_SIZE: usize = 8192;
const DEFAULT_HEADERS: [&'static str; 4] = [
    "User-Agent: fget/0.1.0",
    "Accept: */*",
    "Accept-Encoding: identity",
    "Connection: Keep-Alive",
];

pub type PError = Box<dyn Error>;

struct UrlInfo {
    domain: String,
    addr: String,
    download_path: String,
    fname: String,
    https: bool,
}

struct DownloadInfo(u64, bool);

#[derive(Debug, Clone)]
struct DownloadError(String);

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for DownloadError {}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

fn make_error(err: &str) -> PError {
    Box::new(DownloadError(err.to_string()))
}

fn extract_url_info(url: &str) -> Result<UrlInfo, PError> {
    let parts: Vec<&str> = url.split("/").collect();
    let domain = parts[2];
    let port = if parts[0].starts_with("http") {
        80
    } else {
        443
    };
    let dl_path_start_idx = parts[0].len() + parts[1].len() + 2;

    Ok(UrlInfo {
        domain: domain.to_string(),
        addr: format!("{}:{}", domain, port),
        download_path: url[dl_path_start_idx..].to_string(),
        fname: parts[parts.len() - 1 as usize].to_string(),
        https: port == 443,
    })
}

fn parse_header(header: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = header.split(":").collect();
    if parts.len() != 2 {
        return None;
    }

    Some((String::from(parts[0].trim()), String::from(parts[1].trim())))
}

fn get_download_info<T: ReadWrite>(
    stream: &mut T,
    url_info: &UrlInfo,
) -> Result<DownloadInfo, PError> {
    let req = format!("HEAD {} HTTP/1.1\r\n", url_info.download_path)
        + format!("Host: {}\r\n", url_info.domain).as_str()
        + DEFAULT_HEADERS.join("\r\n").as_str()
        + "\r\n";

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
    if url_info.https {
        let tls_conn = TlsConnector::new()?;
        let stream = TcpStream::connect(&url_info.addr)?;
        let stream = tls_conn.connect(url_info.domain.as_str(), stream)?;
        Ok(Box::new(stream))
    } else {
        let stream = TcpStream::connect(&url_info.addr)?;
        Ok(Box::new(stream))
    }
}

pub fn run(url: &str, opath: &str) -> Result<(), PError> {
    let url_info = extract_url_info(url)?;
    let mut stream = open_conn(&url_info)?;

    let dlinfo = get_download_info(&mut stream, &url_info)?;
    let DownloadInfo(total_size, range_supported) = dlinfo;
    if total_size == 0 {
        return Err(make_error("server return content length is zero"));
    }

    download(&mut stream, &url_info, opath, range_supported)
}
