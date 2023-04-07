use anyhow::{Result, Context};

pub struct HostsConfig {
    data: Vec<u8>,
    pos: usize,
}

impl HostsConfig {

    pub fn new(filename: &str) -> Result<HostsConfig> {
        Ok(HostsConfig {
            data: std::fs::read(filename).with_context(|| format!("read {filename} failed"))?,
            pos: 0,
        })
    }

    pub fn next(&mut self) -> Result<Option<(&str, &str)>> {
        // #[derive(Eq)]
        enum Status { Start, Comment, Ip, IpEnd, Host, HostEnd, LineComment, FmtError }

        let (mut pos, len) = (self.pos, self.data.len());
        let mut status = Status::Start;
        let (mut ip_begin, mut ip_end) = (0, 0);
        let (mut host_begin, mut host_end) = (0, 0);

        while pos < len {
            let c = self.data[pos];

            match status {
                Status::Start => {
                    match c {
                        b'\t' | b' ' | b'\r' | b'\n' => {},
                        b'#' => status = Status::Comment,
                        _ => { status = Status::Ip; ip_begin = pos; },
                    }
                },
                Status::Comment => {
                    match c {
                        b'\r' | b'\n' => status = Status::Start,
                        _ => {},
                    }
                },
                Status::Ip => {
                    match c {
                        b'\t' | b' ' => { status = Status::IpEnd; ip_end = pos; },
                        b'\r' | b'\n' | b'#' => { status = Status::FmtError; break; },
                        _ => {},
                    }
                },
                Status::IpEnd => {
                    match c {
                        b'\t' | b' ' => {},
                        b'\r' | b'\n' | b'#' => { status = Status::FmtError; break; },
                        _ => { status = Status::Host; host_begin = pos; },
                    }
                },
                Status::Host => {
                    match c {
                        b'\t' | b' ' => { status = Status::HostEnd; host_end = pos; },
                        b'\r' | b'\n' => { status = Status::HostEnd; host_end = pos; break; },
                        b'#' => { status = Status::LineComment; host_end = pos; },
                        _ => {},
                    }

                },
                Status::HostEnd => {
                    match c {
                        b'\t' | b' ' => {},
                        b'\r' | b'\n' => break,
                        b'#' => status = Status::LineComment,
                        _ => { status = Status::FmtError; break; },
                    }

                },
                Status::LineComment => {
                    match c {
                        b'\r' | b'\n' => break,
                        _ => {},
                    }
                },
                Status::FmtError => break,
            }

            pos += 1;
        }

        match status {
            Status::Start | Status::Comment => return Ok(None),
            Status::Ip | Status::IpEnd | Status::FmtError => {
                let p = if pos == len { pos } else { pos - 1};
                let line = HostsConfig::location_line(&self.data, p);
                anyhow::bail!("hosts config format error in line {line}");
            },
            Status::Host => host_end = pos,
            _ => {},
        }

        self.pos = if pos == len { pos } else { pos + 1 };

        if let Ok(ip) = std::str::from_utf8(&self.data[ip_begin..ip_end]) {
            if let Ok(host) = std::str::from_utf8(&self.data[host_begin..host_end]) {
                return Ok(Some((host, ip)));
            }
        }

        anyhow::bail!("hosts config format is not utf8");
    }

    fn location_line(data: &[u8], pos: usize) -> usize {
        let len = data.len();
        let max_pos = if pos < len { pos } else { len };
        let mut line = 1;
        for pos in 0..max_pos {
            let c = data[pos];
            if c == b'\n' || (c == b'\r' && (pos + 1 < len && data[pos + 1] != b'\n')) {
                line += 1;
            }
        }

        line
    }

}

#[cfg(test)]
mod tests {
    use super::HostsConfig;

    macro_rules! next_ok {
        ($hc:expr, $host:expr, $ip:expr) => {
            let (h, i) = $hc.next().unwrap().unwrap();
            assert_eq!($host, h);
            assert_eq!($ip, i);
        };
    }

    macro_rules! next_error {
        ($hc:expr) => {
            if let Ok(_) = $hc.next() {
                panic!("expect HostsConfig::next return Err, but is return Ok");
            };
        };
    }

    #[test]
    fn test_hostsconfig() {
        let lines = b"\r\r \n\n \r\n \n\r";
        assert_eq!(7, HostsConfig::location_line(lines, lines.len() - 1));

        fn set_data(hc: &mut HostsConfig, data: &[u8]) {
            hc.data.clear();
            hc.data.extend_from_slice(data);
            hc.pos = 0;
        }

        let mut hc = HostsConfig { data: Vec::new(), pos: 0 };
        assert!(None == hc.next().unwrap());

        set_data(&mut hc, b"  #comment \r\n # comment");
        assert!(None == hc.next().unwrap());

        set_data(&mut hc, b"a");
        next_error!(hc);

        set_data(&mut hc, b"127.0.0.1 a.a.com a");
        next_error!(hc);

        set_data(&mut hc, b"127.0.0.1 a.a.com");
        next_ok!(hc, "a.a.com", "127.0.0.1");

        set_data(&mut hc, b"127.0.0.1 a.a.com\n 127.0.0.2 b.a.com #comment\r  #comment\r\n127.0.0.3 c.a.com  \n 1 2 \n 3 4 5");
        next_ok!(hc, "a.a.com", "127.0.0.1");
        next_ok!(hc, "b.a.com", "127.0.0.2");
        next_ok!(hc, "c.a.com", "127.0.0.3");
        next_ok!(hc, "2", "1");
        next_error!(hc);
    }

}
