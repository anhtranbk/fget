use fget::{make_error, PError};

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
        if parts.len() < 4 {
            return Err(make_error("Invalid URL"));
        }

        let scheme = parse_and_validate_scheme(&parts[0])?;
        let (host, port) = parse_host_and_port(parts[2], scheme)?;
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

fn parse_host_and_port<'a>(addr: &'a str, scheme: &str) -> Result<(&'a str, u16), PError> {
    if addr.contains(":") {
        let parts: Vec<&str> = addr.split(":").collect();
        if parts.len() != 2 {
            return Err(make_error("Invalid address"));
        }

        Ok((parts[0], parts[1].parse::<u16>()?))
    } else {
        let port = match scheme {
            "http" => 80,
            "https" => 443,
            _ => return Err(make_error("Invalid scheme")),
        };

        Ok((addr, port))
    }
}

fn parse_and_validate_scheme(scheme: &str) -> Result<&str, PError> {
    if scheme == "http:" || scheme == "https:" {
        Ok(&scheme[..scheme.len() - 1])
    } else {
        Err(make_error("Invalid scheme"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url() {
        let url = "https://download.virtualbox.org/virtualbox/7.0.8/VirtualBox-7.0.8_BETA4-156879-macOSArm64.dmg";
        let urlinfo = UrlInfo::parse(url).unwrap();

        assert_eq!("https", urlinfo.scheme.as_str());
        assert_eq!("download.virtualbox.org", urlinfo.domain.as_str());
        assert_eq!(
            "/virtualbox/7.0.8/VirtualBox-7.0.8_BETA4-156879-macOSArm64.dmg",
            urlinfo.path.as_str()
        );
        assert_eq!(
            "VirtualBox-7.0.8_BETA4-156879-macOSArm64.dmg",
            urlinfo.fname.as_str()
        );
        assert_eq!(true, urlinfo.is_tls());
        assert_eq!(443, urlinfo.port);
        assert_eq!("download.virtualbox.org:443", urlinfo.host_addr());
    }

    #[test]
    fn test_parse_url_custom_port() {
        let url = "http://localhost:8080/download/GoTiengViet.dmg";
        let urlinfo = UrlInfo::parse(url).unwrap();

        assert_eq!("http", urlinfo.scheme.as_str());
        assert_eq!("localhost", urlinfo.domain.as_str());
        assert_eq!("/download/GoTiengViet.dmg", urlinfo.path.as_str());
        assert_eq!("GoTiengViet.dmg", urlinfo.fname.as_str());
        assert_eq!(false, urlinfo.is_tls());
        assert_eq!(8080, urlinfo.port);
        assert_eq!("localhost:8080", urlinfo.host_addr());
    }
}
