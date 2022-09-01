use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    str::{self, FromStr},
    time::Duration,
};

use http::{header, request::Builder, Method, Request, Response, StatusCode};
use native_tls::TlsConnector;

use fget::{hash_map, make_error, PError};

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
        let scheme = &parts[0][..parts[0].len() - 1];
        let host = parts[2];
        let port = match scheme {
            "http" => 80,
            "https" => 443,
            _ => return Err(make_error("Invalid scheme")),
        };

        let query_idx = parts[0].len() + parts[1].len() + parts[2].len() + 2;
        Ok(UrlInfo {
            scheme: scheme.to_string(),
            domain: host.to_string(),
            port,
            path: url[query_idx..].to_string(),
            fname: parts[parts.len() - 1].to_string(),
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

pub type HttpBody = BufReader<ReadWrapper>;
pub type HttpResponse = Response<HttpBody>;
pub type HttpHeaders = HashMap<String, String>;

// static DEFAULT_HEADERS: HashMap<&str, &str> = hash_map!(
//     "User-Agent" => "fget/0.1.0",
//     "Accept" => "*/*",
//     "Accept-Encoding" => "identity",
//     "Connection" => "Keep-Alive"
// );

const DEFAULT_TIMEOUT_MS: u64 = 5 * 1000;
const DEFAULT_REDIRECT_POLICY: RedirectPolicy = RedirectPolicy::Follow(10);

/// One-time http client
pub struct HttpClient {
    host_addr: String,
    rw: Option<Box<dyn ReadWrite>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum RedirectPolicy {
    Follow(u8), // maximum number of redirects
    None,       // do not follow redirects
}

#[derive(Debug, Clone)]
pub struct HttpConfig {
    redirect_policy: RedirectPolicy,
    timeout_ms: u64,
}

#[allow(dead_code)]
impl HttpClient {
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder::new()
    }

    pub fn connect(
        host_addr: &str,
        domain: &str,
        tls: bool,
        cfg: &HttpConfig,
    ) -> Result<Self, PError> {
        Ok(Self {
            host_addr: host_addr.to_string(),
            rw: Some(open_conn(host_addr, domain, tls, cfg.timeout_ms)?),
        })
    }

    /// send a head request, because of one-time so client will be moved out after this method
    pub fn head(mut self, path: &str) -> Result<HttpResponse, PError> {
        let req = self
            .make_request(Method::HEAD, path, None)
            .body(vec![])
            .unwrap();
        self.send_request(&req)
    }

    /// send a head request with custom headers, because of one-time,
    /// so client will be moved out after this method
    pub fn head_with_headers(
        mut self,
        path: &str,
        headers: &HttpHeaders,
    ) -> Result<HttpResponse, PError> {
        let req = self
            .make_request(Method::HEAD, path, Some(headers))
            .body(vec![])
            .unwrap();
        self.send_request(&req)
    }

    /// send a get request, because of one-time, so client will be moved out after this method
    pub fn get(mut self, path: &str) -> Result<HttpResponse, PError> {
        let req = self
            .make_request(Method::GET, path, None)
            .body(vec![])
            .unwrap();
        self.send_request(&req)
    }

    /// send a get request with custom headers, because of one-time,
    /// so client will be moved out after this method
    pub fn get_with_headers(
        mut self,
        path: &str,
        headers: &HttpHeaders,
    ) -> Result<HttpResponse, PError> {
        let req = self
            .make_request(Method::GET, path, Some(headers))
            .body(vec![])
            .unwrap();
        self.send_request(&req)
    }

    fn make_request(&self, method: Method, path: &str, headers: Option<&HttpHeaders>) -> Builder {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, &self.host_addr);

        if let Some(headers) = headers {
            for (key, val) in headers.iter() {
                builder = builder.header(key, val);
            }
        }

        let default_headers: HashMap<&str, &str> = hash_map!(
            "User-Agent" => "fget/0.1.0",
            "Accept" => "*/*",
            "Accept-Encoding" => "identity",
            "Connection" => "Keep-Alive"
        );

        for (key, val) in default_headers.iter() {
            builder = builder.header(*key, *val);
        }

        builder
    }

    fn send_request(&mut self, req: &Request<Vec<&u8>>) -> Result<Response<HttpBody>, PError> {
        let mut data = format!("{} {} HTTP/1.1\r\n", req.method(), req.uri());
        for (key, val) in req.headers().iter() {
            data += &key.to_string();
            data += ": ";
            data += &val.to_str().unwrap();
            data += "\r\n";
        }
        // end of headers
        data += "\r\n";

        let mut rw = self.rw.take().unwrap();
        rw.write_all(data.as_bytes())?;
        rw.flush()?;

        let br = BufReader::new(ReadWrapper(rw));
        Ok(HttpClient::make_response(br)?)
    }

    fn make_response(mut br: BufReader<ReadWrapper>) -> Result<HttpResponse, PError> {
        let mut buf = String::new();
        br.read_line(&mut buf)?;

        let parts: Vec<&str> = buf.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(make_error("invalid response"));
        }

        let status_code = StatusCode::from_str(parts[1])?;
        if status_code.as_u16() / 100 >= 4 {
            return Err(make_error(
                format!("server response error: {}", status_code.as_u16(),).as_str(),
            ));
        }

        let mut builder = Response::builder().status(status_code);
        buf.clear();

        // read_line may block forever if no endline found
        while br.read_line(&mut buf)? > 2 {
            // len > 2 because read_line always includes \r\n
            if let Some((key, val)) = parse_header(&buf.trim_end()) {
                builder = builder.header(key, val);
            }
            buf.clear();
        }

        Ok(builder.body(br).unwrap())
    }
}

pub struct HttpClientBuilder {
    host_addr: String,
    tls: bool,
    domain: String,
    cfg: HttpConfig,
}

#[allow(dead_code)]
impl HttpClientBuilder {
    pub fn new() -> HttpClientBuilder {
        HttpClientBuilder {
            host_addr: String::new(),
            tls: false,
            domain: String::new(),
            cfg: HttpConfig {
                redirect_policy: DEFAULT_REDIRECT_POLICY,
                timeout_ms: DEFAULT_TIMEOUT_MS,
            },
        }
    }

    pub fn from(self, url: &str) -> Result<HttpClientBuilder, PError> {
        Ok(self.from_url_info(&UrlInfo::parse(url)?))
    }

    pub fn from_url_info(mut self, url_info: &UrlInfo) -> HttpClientBuilder {
        self.host_addr = url_info.host_addr();
        self.tls = url_info.is_tls();

        self.domain.clear();
        self.domain += &url_info.domain;

        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> HttpClientBuilder {
        self.cfg.timeout_ms = timeout_ms;
        self
    }

    pub fn with_redirect_policy(mut self, policy: RedirectPolicy) -> HttpClientBuilder {
        self.cfg.redirect_policy = policy;
        self
    }

    pub fn with_host_addr(mut self, addr: &str) -> HttpClientBuilder {
        self.host_addr.clear();
        self.host_addr.push_str(addr);
        self
    }

    pub fn with_tls(mut self, domain: &str) -> HttpClientBuilder {
        self.domain.clear();
        self.domain.push_str(domain);
        self
    }

    pub fn build(self) -> Result<HttpClient, PError> {
        if self.host_addr.is_empty() {
            return Err(make_error("no host_addr specified"));
        }

        HttpClient::connect(
            self.host_addr.as_str(),
            self.domain.as_str(),
            self.tls,
            &self.cfg,
        )
    }
}

#[allow(dead_code)]
pub fn head(url: &str) -> Result<HttpResponse, PError> {
    let ui = UrlInfo::parse(url)?;
    HttpClient::builder()
        .from_url_info(&ui)
        .build()?
        .head(&ui.path)
}

#[allow(dead_code)]
pub fn get(url: &str) -> Result<HttpResponse, PError> {
    let ui = UrlInfo::parse(url)?;
    HttpClient::builder()
        .from_url_info(&ui)
        .build()?
        .get(&ui.path)
}

fn open_conn(
    host_addr: &str,
    domain: &str,
    tls: bool,
    timeout_ms: u64,
) -> Result<Box<dyn ReadWrite>, PError> {
    let duration = Duration::from_millis(timeout_ms);
    let sock_addr = resolve_addr(host_addr)?;
    let stream = TcpStream::connect_timeout(&sock_addr, duration)?;

    if tls {
        let tls_conn = TlsConnector::new()?;
        let stream = tls_conn.connect(domain, stream)?;
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

#[cfg(test)]
mod tests {
    use crate::httpx::UrlInfo;

    #[test]
    fn test_parse_url() {
        let url = "https://download.virtualbox.org/virtualbox/7.0.8/VirtualBox-7.0.8_BETA4-156879-macOSArm64.dmg";
        let url_info = UrlInfo::parse(url).unwrap();

        assert_eq!("https", url_info.scheme.as_str());
        assert_eq!("download.virtualbox.org", url_info.domain.as_str());
        assert_eq!(
            "/virtualbox/7.0.8/VirtualBox-7.0.8_BETA4-156879-macOSArm64.dmg",
            url_info.path.as_str()
        );
        assert_eq!(
            "VirtualBox-7.0.8_BETA4-156879-macOSArm64.dmg",
            url_info.fname.as_str()
        );
        assert_eq!(true, url_info.is_tls());
        assert_eq!(443, url_info.port);
    }
}
