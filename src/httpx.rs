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

use crate::urlinfo::UrlInfo;

pub trait ReadWrite: Read + Write {}

impl<T: Read + Write> ReadWrite for T {}

pub struct ToRead(Box<dyn ReadWrite>);

impl Read for ToRead {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

pub type HttpBody = BufReader<ToRead>;
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
const DEFAULT_USER_AGENT: &'static str = "fget/0.1.0";

/// One-time http client
pub struct HttpClient {
    host_addr: String,
    rw: Option<Box<dyn ReadWrite>>,
    cfg: HttpConfig,
}

#[derive(Debug, Clone)]
pub enum RedirectPolicy {
    Follow(u8), // maximum number of redirects
    None,       // do not follow redirects
}

#[derive(Debug, Clone)]
pub struct HttpConfig {
    redirect_policy: RedirectPolicy,
    timeout_ms: u64,
    user_agent: String,
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
            cfg: cfg.clone(),
        })
    }

    /// send a head request, because of one-time so client will be moved out after this method
    pub fn head(mut self, path: &str) -> Result<HttpResponse, PError> {
        let req = self.make_request(Method::HEAD, path, None).body(vec![])?;
        self.send(&req)
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
            .body(vec![])?;

        self.send(&req)
    }

    /// send a get request, because of one-time, so client will be moved out after this method
    pub fn get(mut self, path: &str) -> Result<HttpResponse, PError> {
        let req = self.make_request(Method::GET, path, None).body(vec![])?;
        self.send(&req)
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
            .body(vec![])?;

        self.send(&req)
    }

    fn make_request(&self, method: Method, path: &str, headers: Option<&HttpHeaders>) -> Builder {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, &self.host_addr)
            .header(header::USER_AGENT, &self.cfg.user_agent);

        let default_headers: HashMap<&str, &str> = hash_map!(
            "Accept" => "*/*",
            "Accept-Encoding" => "identity",
            "Connection" => "Keep-Alive"
        );

        if let Some(headers) = headers {
            for (key, val) in headers.iter() {
                builder = builder.header(key, val);
            }
        }

        for (key, val) in default_headers.iter() {
            builder = builder.header(*key, *val);
        }

        builder
    }

    fn send(mut self, req: &Request<Vec<&u8>>) -> Result<HttpResponse, PError> {
        if req.method() != Method::GET && req.method() != Method::HEAD {
            return Err(make_error("unsupported method"));
        }

        let mut data = format!("{} {} HTTP/1.1\r\n", req.method(), req.uri());
        for (key, val) in req.headers().iter() {
            data += &key.to_string();
            data += ": ";
            data += &val.to_str()?;
            data += "\r\n";
        }
        // end of headers
        data += "\r\n";

        let mut rw = self.rw.take().unwrap();
        rw.write_all(data.as_bytes())?;
        rw.flush()?;

        Ok(self.make_response(req, BufReader::new(ToRead(rw)))?)
    }

    fn get_status_code(&self, br: &mut BufReader<ToRead>) -> Result<StatusCode, PError> {
        let mut buff = String::new();
        br.read_line(&mut buff)?;

        let parts: Vec<&str> = buff.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(make_error("invalid response"));
        }

        Ok(StatusCode::from_str(parts[1])?)
    }

    fn make_response<T>(
        self,
        req: &Request<T>,
        mut br: BufReader<ToRead>,
    ) -> Result<HttpResponse, PError> {
        let status_code = self.get_status_code(&mut br)?;
        if status_code.as_u16() / 100 >= 4 {
            return Err(make_error(
                format!("server response error: {}", status_code.as_u16(),).as_str(),
            ));
        }
        if status_code.as_u16() / 100 == 3 {
            match self.cfg.redirect_policy {
                RedirectPolicy::None => return Err(make_error("redirect is not supported")),
                RedirectPolicy::Follow(max_redirects) => {
                    return if max_redirects > 0 {
                        self.handle_redirect(req, &status_code, br, max_redirects)
                    } else {
                        Err(make_error("max redirects exceeded"))
                    }
                }
            }
        }

        let mut builder = Response::builder().status(status_code);
        for (key, val) in HeaderIterator::from(&mut br) {
            builder = builder.header(key, val);
        }

        Ok(builder.body(br)?)
    }

    fn handle_redirect<T>(
        self,
        req: &Request<T>,
        status_code: &StatusCode, // only for logging purposes
        mut br: BufReader<ToRead>,
        max_redirects: u8,
    ) -> Result<HttpResponse, PError> {
        for (key, val) in HeaderIterator::from(&mut br) {
            let key = key.to_lowercase();
            if key.trim() == "location" {
                println!("Redirecting to: {}", val);
                // build new client with same config from current one
                let client = HttpClientBuilder::new()
                    .from_url(&val)?
                    .with_config(&self.cfg)
                    .with_timeout_ms(self.cfg.timeout_ms)
                    .with_redirect_policy(RedirectPolicy::Follow(max_redirects - 1))
                    .build()?;

                match *req.method() {
                    Method::GET => return client.get(&val),
                    Method::HEAD => return client.head(&val),
                    _ => return Err(make_error("unsupported method")),
                }
            }
        }

        Err(make_error(
            format!(
                "server return {} but no location header was found",
                status_code.as_u16()
            )
            .as_str(),
        ))
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
                user_agent: DEFAULT_USER_AGENT.to_string(),
            },
        }
    }

    pub fn from_url(self, url: &str) -> Result<HttpClientBuilder, PError> {
        Ok(self.from_url_info(&UrlInfo::parse(url)?))
    }

    pub fn from_url_info(mut self, urlinfo: &UrlInfo) -> HttpClientBuilder {
        self.host_addr = urlinfo.host_addr();
        self.tls = urlinfo.is_tls();

        self.domain.clear();
        self.domain += &urlinfo.domain;

        self
    }

    pub fn with_config(mut self, cfg: &HttpConfig) -> HttpClientBuilder {
        self.cfg = cfg.clone();
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

    pub fn with_user_agent(mut self, ua: &str) -> HttpClientBuilder {
        self.cfg.user_agent.clear();
        self.cfg.user_agent += ua;

        self
    }

    pub fn with_host_addr(mut self, addr: &str) -> HttpClientBuilder {
        self.host_addr.clear();
        self.host_addr += addr;

        self
    }

    pub fn with_tls(mut self, domain: &str) -> HttpClientBuilder {
        self.tls = true;
        self.domain.clear();
        self.domain += domain;

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

struct HeaderIterator<'a> {
    br: &'a mut BufReader<ToRead>,
    buf: String,
}

impl HeaderIterator<'_> {
    fn from(br: &mut BufReader<ToRead>) -> HeaderIterator {
        HeaderIterator {
            br,
            buf: String::new(),
        }
    }
}

impl Iterator for HeaderIterator<'_> {
    type Item = (String, String);

    fn next(&mut self) -> Option<Self::Item> {
        self.buf.clear();

        // read_line may block forever if no endline found
        if let Ok(n) = self.br.read_line(&mut self.buf) {
            // len > 2 because read_line always includes \r\n
            if n > 2 {
                return parse_header(&self.buf.trim_end())
                    .map(|(key, val)| (key.to_string(), val.to_string()));
            }
        }

        None
    }
}

pub fn resolve_addr(addr: &str) -> Result<SocketAddr, PError> {
    let mut sock_addrs = addr.to_socket_addrs()?;
    if sock_addrs.len() == 0 {
        return Err(make_error("no valid host address found"));
    }

    let sock_addr = sock_addrs.next().unwrap();
    let sock_addr = sock_addrs
        .filter(|ip| ip.is_ipv4())
        .next()
        .unwrap_or_else(|| sock_addr); // try to use ipv4 address if available

    Ok(sock_addr)
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
    let dur = Duration::from_millis(timeout_ms);
    let sock_addr = resolve_addr(host_addr)?;

    let stream = TcpStream::connect_timeout(&sock_addr, dur)?;
    stream.set_read_timeout(Some(dur))?;
    stream.set_write_timeout(Some(dur))?;

    if tls {
        let tls_conn = TlsConnector::new()?;
        let stream = tls_conn.connect(domain, stream)?;
        Ok(Box::new(stream))
    } else {
        Ok(Box::new(stream))
    }
}

fn parse_header(header: &str) -> Option<(&str, &str)> {
    if let Some(pos) = header.find(':') {
        Some((&header[..pos].trim(), &header[pos + 1..].trim()))
    } else {
        None
    }
}
