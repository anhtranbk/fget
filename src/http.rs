use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    str::FromStr,
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

type HttpBody = BufReader<ReadWrapper>;

// static DEFAULT_HEADERS: HashMap<&str, &str> = hash_map!(
//     "User-Agent" => "fget/0.1.0",
//     "Accept" => "*/*",
//     "Accept-Encoding" => "identity",
//     "Connection" => "Keep-Alive"
// );

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

    pub fn head(&mut self, path: &str) -> Result<Response<HttpBody>, PError> {
        let req = self.make_request(Method::HEAD, path).body(vec![]).unwrap();
        self.send_request(&req)
    }

    pub fn get(&mut self, path: &str) -> Result<Response<HttpBody>, PError> {
        let req = self.make_request(Method::GET, path).body(vec![]).unwrap();
        self.send_request(&req)
    }

    fn make_request(&self, method: Method, path: &str) -> Builder {
        let mut builder = Request::builder()
            .method(method)
            .uri(format!("{}", path))
            .header(header::HOST, &self.url_info.domain);

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

    fn make_response(mut br: BufReader<ReadWrapper>) -> Result<Response<HttpBody>, PError> {
        let mut buf = String::new();
        br.read_line(&mut buf)?;

        let parts: Vec<&str> = buf.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(make_error("invalid response"));
        }

        let status_code = StatusCode::from_str(parts[1])?;
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

fn open_conn(url_info: &UrlInfo) -> Result<Box<dyn ReadWrite>, PError> {
    // TODO: allow user to configure duration
    let duration = Duration::from_secs(5);
    let stream = TcpStream::connect_timeout(
        &url_info.host_addr().to_socket_addrs()?.next().unwrap(),
        duration,
    )?;

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

#[cfg(test)]
mod tests {
    use crate::http::UrlInfo;

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
