use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
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

type HttpBody = Box<dyn Read>;

pub trait ReadWrite: Read + Write {}

impl<T: Read + Write> ReadWrite for T {}

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

    fn request(&self, req: &Request<HttpBody>) -> Result<Response<HttpBody>, PError> {
        panic!()
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

fn parse_header(header: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = header.split(":").collect();
    if parts.len() != 2 {
        return None;
    }

    Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
}
