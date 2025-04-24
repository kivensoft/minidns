use std::net::UdpSocket;

/// 应用程序内部名称
const APP_NAME: &str = "mini dns client";
/// 应用程序版本
const APP_VER: &str = include_str!(concat!(env!("OUT_DIR"), "/.version"));
/// 魔数
const C_MAGIC: &str = "kdns";
/// 瞬时时间起点
const C_2023_01_01: u64 = 1672531200;

appcfg::appconfig_define!(app_conf, AppConf,
    debug : bool   => ["D",  "debug", "", "set debug mode"],
    domain: String => ["n",  "domain", "DOMAIN", "set dynamic domain name"],
    ip    : String => ["i",  "ip", "IP", "set dynamic ip address"],
    key   : String => ["k",  "key", "KEY", "set dynamic updated key"],
    dns   : String => ["d",  "dns", "DNS", "set dynamic dns server address"]
);

impl Default for AppConf {
    fn default() -> Self {
        AppConf {
            debug  : false,
            domain : String::new(),
            ip     : String::from("0.0.0.0"),
            key    : String::new(),
            dns    : String::new(),
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

fn now_of_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

fn main() {
    let version = format!("{APP_NAME} version {APP_VER} CopyLeft Kivensoft 2015-2023.");
    let mut ac = AppConf::default();
    if !appcfg::parse_args(&mut ac, &version).unwrap() {
        return;
    }
    let ac = ac.init();
    if ac.debug {
        unsafe { DEBUG = true; }
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

    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.set_read_timeout(Some(std::time::Duration::new(5, 0))).unwrap();
    socket.set_write_timeout(Some(std::time::Duration::new(5, 0))).unwrap();

    let dns_addr = format!("{}:53", ac.dns);
    let mut buf = [0; 512];

    dbg_out!("send packet to {}, message = {}", ac.dns, packet);
    socket.send_to(packet.as_bytes(), dns_addr).unwrap();
    let (nread, addr) = socket.recv_from(&mut buf).unwrap();
    let rep_msg = String::from_utf8_lossy(&buf[..nread]);
    dbg_out!("receive from {}, nread = {}, message = {}", addr, nread, rep_msg);
    println!("{}", rep_msg);
}
