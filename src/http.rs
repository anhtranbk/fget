use std::{
    io::{BufReader, BufWriter, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    rc::Rc,
};

use http::{HeaderMap, Request, Response};
use native_tls::TlsConnector;

use fget::{make_error, PError};

const DEFAULT_HEADERS: [&'static str; 4] = [
    "User-Agent: fget/0.1.0",
    "Accept: */*",
    "Accept-Encoding: identity",
    "Connection: Keep-Alive",
];

pub struct ReadWrapper(Box<dyn ReadWrite>);

pub trait ReadWrite: Read + Write {}

impl<T: Read + Write> ReadWrite for T {}

impl Read for ReadWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

#[derive(Debug, Clone)]
pub struct UrlInfo {
    pub scheme: String,
    pub domain: String,
    pub port: u16,
    pub path: String,
    pub fname: String,
}

impl UrlInfo {
    pub fn parse(url: &str) -> Result<UrlInfo, PError> {
        let parts: Vec<&str> = url.split("/").collect();
        let scheme = parts[0];
        let host = parts[2];
        let port = if parts[0].starts_with("http") {
            80
        } else {
            443
        };

        let query_idx = parts[0].len() + parts[1].len() + 2;
        Ok(UrlInfo {
            scheme: scheme.to_string(),
            domain: host.to_string(),
            port,
            path: url[query_idx..].to_string(),
            fname: parts[parts.len() - 1 as usize].to_string(),
        })
    }

    pub fn host_addr(&self) -> String {
        format!("{}:{}", self.domain, self.port)
    }

    pub fn is_tls(&self) -> bool {
        self.scheme == "https"
    }
}

pub fn resolve_addr(addr: &str) -> Result<SocketAddr, PError> {
    let mut sock_addrs = addr.to_socket_addrs()?;
    if sock_addrs.len() == 0 {
        return Err(make_error("invalid host address"));
    }

    let sock_addr = sock_addrs.next().unwrap();
    let sock_addr = sock_addrs
        .filter(|ip| ip.is_ipv4())
        .next()
        .unwrap_or_else(|| sock_addr); // try to use ipv4 address if available

    Ok(sock_addr)
}

type HttpBody = BufReader<ReadWrapper>;

/// One-time http client
pub struct HttpClient {
    url_info: UrlInfo,
    rw: Option<Box<dyn ReadWrite>>,
}

impl HttpClient {
    pub fn connect(url_info: &UrlInfo) -> Result<Self, PError> {
        Ok(Self {
            url_info: url_info.clone(),
            rw: Some(open_conn(&url_info)?),
        })
    }

    pub fn head(&self, path: &str) -> Result<Response<HttpBody>, PError> {
        panic!()
    }

    pub fn get(&self, path: &str) -> Result<Response<HttpBody>, PError> {
        panic!()
    }

    fn request(&mut self, req: &Request<Vec<&u8>>) -> Result<Response<HttpBody>, PError> {
        let req = format!("{} {} HTTP/1.1\r\n", req.method(), req.uri())
            + format!("Host: {}\r\n", self.url_info.domain).as_str()
            + DEFAULT_HEADERS.join("\r\n").as_str()
            + "\r\n\r\n";

        let mut rw = self.rw.take().unwrap();
        rw.write_all(req.as_bytes())?;

        let r = ReadWrapper(rw);
        let br = BufReader::new(r);

        // extract headers

        let resp = Response::builder()
            .status(200)
            .header("", "")
            .body(br)
            .unwrap();

        Ok(resp)
    }
}

fn extract_headers<T: Read>(r: &T) -> HeaderMap {
    let headers = HeaderMap::new();

    headers
}

fn open_conn(url_info: &UrlInfo) -> Result<Box<dyn ReadWrite>, PError> {
    let stream = TcpStream::connect(&url_info.host_addr())?;
    if url_info.is_tls() {
        let tls_conn = TlsConnector::new()?;
        let stream = tls_conn.connect(url_info.domain.as_str(), stream)?;
        Ok(Box::new(stream))
    } else {
        Ok(Box::new(stream))
    }
}

// struct RW(Rc<dyn Read>, Rc<dyn Write>);

// fn open_conn2(url_info: &UrlInfo) -> Result<RW, PError> {
//     let mut stream = TcpStream::connect(&url_info.host_addr())?;
//     if url_info.is_tls() {
//         let tls_conn = TlsConnector::new()?;
//         let stream = tls_conn.connect(url_info.domain.as_str(), stream)?;
//         Ok(RW(Rc::new(stream), Rc::clone(&stream)))
//     } else {
//         Ok(RW(Rc::new(stream), Rc::clone(&stream)))
//     }
// }

fn parse_header(header: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = header.split(":").collect();
    if parts.len() != 2 {
        return None;
    }

    Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
}
