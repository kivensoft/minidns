
pub struct HostsConfig {
    data: Vec<u8>,
    pos: usize,
    line: usize,
}

// pub struct HostsConfigIter<'a> {
//     data: &'a [u8],
//     pos: usize,
//     line: usize,
//     err: Option<String>,
// }

impl HostsConfig {

    pub fn new(filename: &str) -> Result<HostsConfig, std::io::Error> {
        Ok(HostsConfig { data: std::fs::read(filename)?, pos: 0, line: 0, })
    }

    pub fn next(&mut self) -> Result<Option<(&str, &str)>, String> {
        // #[derive(Eq)]
        enum Status { Start, Comment, Ip, HostBegin, Host, EndComment }

        let (mut pos, len) = (0, self.data.len());
        let mut status = Status::Start;
        let (mut ip_begin, mut ip_end) = (0, 0);
        let (mut host_begin, mut host_end) = (0, 0);
        
        for i in self.pos..len {
            pos = i;
            match self.data[i] {
                // 空白符
                0x09 | 0x20 => {
                    match status {
                        Status::Ip => { ip_end = i; status = Status::HostBegin; },
                        Status::Host => host_end = i,
                        _ => {},
                    }
                },
                // 回车换行
                c @ (0x0a | 0x0d) => {
                    if !(c == 0x0a && i > 0 && self.data[i - 1] == 0x0d) {
                        self.line += 1;
                    }
                    match status {
                        Status::Start => {}, 
                        Status::Comment => status = Status::Start,
                        Status::Host => { host_end = i; break; }
                        _ => break,
                    }
                },
                0x23 => match status {
                        Status::Ip => { ip_end = i; status = Status::EndComment; },
                        Status::Host => { host_end = i; status = Status::EndComment; },
                        _ => status = Status::Comment,
                },
                _ => {
                    match status {
                        Status::Start => { ip_begin = i; status = Status::Ip; },
                        Status::HostBegin => { host_begin = i; status = Status::Host; },
                        _ => {},
                    }
                }
            }
        }

        self.pos = pos + 1;

        if pos == len - 1 && host_begin > 0 && host_end == 0 {
            host_end = pos;
        }

        if ip_begin == 0 && ip_end == 0 {
            return Ok(None);
        }

        if ip_begin >= ip_end || host_begin >= host_end {
            return Err(format!("hosts config format error in line {}", self.line));
        }

        if let Ok(ip) = std::str::from_utf8(&self.data[ip_begin..ip_end]) {
            if let Ok(host) = std::str::from_utf8(&self.data[host_begin..host_end]) {
                println!("host = {}, ip = {}", host, ip);
                return Ok(Some((host, ip)));
            }
        }

        Err("hosts config format is not utf8".to_owned())
    }
}

