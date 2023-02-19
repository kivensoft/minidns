use std::net::{UdpSocket, Ipv4Addr};
use getopts::Options;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const C_MAGIC: &str = "kdns";
const C_2023_01_01: u64 = 1672531200;

const C_HELP  : &str = "help";
const C_DOMAIN: &str = "domain";
const C_IP    : &str = "ip";
const C_KEY   : &str = "key";
const C_DNS   : &str = "dns";
const C_DEBUG : &str = "debug";

#[derive(Debug)]
struct AppConf {
    debug : bool,
    domain: String,
    ip    : String,
    key   : String,
    dns   : String,
}

impl Default for AppConf {
    fn default() -> Self {
        AppConf {
            debug : false,
            domain: String::new(),
            ip    : String::new(),
            key   : String::new(),
            dns   : String::new(),
        }
    }
}

static mut DEBUG: bool = false;

macro_rules! dbg_out {
    ($($arg:tt)*) => {{
        if unsafe { DEBUG } {
            println!($($arg)*);
        }
    }};
}

macro_rules! get_opt_str {
    ($matches: expr, $name: expr, $out_val: expr) => {
        if let Some(s) = $matches.opt_str($name) {
            $out_val = s;
        }
    };
}

fn now_of_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

fn print_usage(prog: &str, opts: &Options) {
    let path = std::path::Path::new(prog);
    let prog = path.file_name().unwrap().to_str().unwrap();
    let brief = format!("Usage: {} [options]", prog);
    println!("{}", opts.usage(&brief));
}

fn parse_cmdline(ac: &mut AppConf) -> Result<bool> {
    let mut args = std::env::args();
    let prog = args.next().unwrap();
    let mut opts = Options::new();

    opts.optflag("", C_HELP, "print this help menu");
    opts.optopt("h", C_DOMAIN, "set dynamic domain name", "DOMAIN");
    opts.optopt("i", C_IP, "set dynamic ip address", "IP");
    opts.optopt("k", C_KEY, "set dynamic updated key", "KEY");
    opts.optopt("d", C_DNS, "set dynamic dns server address", "DNS");
    opts.optflag("D", C_DEBUG, "set debug mode");

    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(e) => return Err(e.to_string().into()),
    };

    if matches.opt_present(C_HELP) {
        print_usage(&prog, &opts);
        return Ok(false);
    }

    if matches.opt_present(C_DEBUG) {
        ac.debug = true;
        unsafe { DEBUG = true; }
    }
    
    get_opt_str!(matches, C_DOMAIN, ac.domain);
    get_opt_str!(matches, C_IP, ac.ip);
    get_opt_str!(matches, C_KEY, ac.key);
    get_opt_str!(matches, C_DNS, ac.dns);

    if ac.domain.is_empty() || ac.dns.is_empty() {
        print_usage(&prog, &opts);
        return Ok(false);
    }

    if ac.ip.is_empty() {
        ac.ip = "0.0.0.0".to_owned();
    }

    if let Err(_) = ac.ip.parse::<Ipv4Addr>() {
        return Err("arg ip format error".into());
    }

    Ok(true)
}

fn main() -> Result<()> {
    let mut ac = AppConf::default();
    if !parse_cmdline(&mut ac)? {
        return Ok(())
    }
    dbg_out!("application config setting: {:#?}", ac);

    let id = now_of_unix() - C_2023_01_01;
    let digest = {
        let mut ctx = md5::Context::new();
        ctx.consume(id.to_string().as_bytes());
        ctx.consume(ac.domain.as_bytes());
        ctx.consume(ac.ip.as_bytes());
        ctx.consume(ac.key.as_bytes());
        format!("{:x}", ctx.compute())
    };

    dbg_out!("MAGIC = {}, DIGEST = {}, ID = {}, DOMAIN = {}, IP = {}",
            C_MAGIC, digest, id, ac.domain, ac.ip);
    let packet = format!("{} {} {} {} {}", C_MAGIC, digest, id, ac.domain, ac.ip);

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(std::time::Duration::new(5, 0)))?;
    socket.set_write_timeout(Some(std::time::Duration::new(5, 0)))?;
    
    let dns_addr = format!("{}:53", ac.dns);
    let mut buf = [0; 512];

    dbg_out!("send packet to {}, message = {}", ac.dns, packet);
    socket.send_to(packet.as_bytes(), dns_addr)?;
    let (nread, addr) = socket.recv_from(&mut buf)?;
    let rep_msg = String::from_utf8_lossy(&buf[..nread]);
    dbg_out!("receive from {}, nread = {}, message = {}", addr, nread, rep_msg);
    println!("{}", rep_msg);

    Ok(())
}
