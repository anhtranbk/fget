use native_tls::TlsConnector;
use std::{
    error::Error,
    io::{Read, Write},
    net::TcpStream,
    str,
};

pub fn download(url: &str) -> Result<(), Box<dyn Error>> {
    let domain = url.split("/").nth(2).unwrap();
    let host = format!("{}:{}", domain, 443);
    let addr = &url[(8 + domain.len())..];

    println!("connecting to {} at {}", host, addr);

    let tls_conn = TlsConnector::new()?;

    let stream = TcpStream::connect(host)?;
    let mut stream = tls_conn.connect(domain, stream)?;
    let mut buf = [0u8; 8192];

    let req = format!(
        concat!(
            "GET {} HTTP/1.1\r\n",
            "Host: {}:443\r\n",
            "User-Agent: Wget/1.21.4\r\n",
            "Accept: */*\r\n",
            "Accept-Encoding: identity\r\n",
            "Connection: Keep-Alive\r\n\r\n",
        ),
        addr, domain
    );
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
