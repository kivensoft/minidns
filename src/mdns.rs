mod bufutil;
mod dnsutil;
mod dnsserver;
mod hostsconf;

use dnsserver::*;
use hostsconf::*;

const APP_NAME: &str = "mini dns server";   // 应用程序内部名称
// 应用程序版本
const APP_VER: &str = include_str!(concat!(env!("OUT_DIR"), "/.version"));
const G_BANNER: &str = r##"
   Kivensoft  _       _     __ ver ?
   ____ ___  (_)___  (_)___/ /___  _____
  / __ `__ \/ / __ \/ / __  / __ \/ ___/
 / / / / / / / / / / / /_/ / / / (__  )
/_/ /_/ /_/_/_/ /_/_/\__,_/_/ /_/____/
"##;

appcfg::appconfig_define!(app_conf, AppConf,
    log_level : String => ["L",  "log-level",  "LOG_LEVEL", "set log level(trace/debug/info/warn/error/off)"],
    log_file  : String => ["F",  "log-file",   "LOG_FILE", "set log file path"],
    log_max   : String => ["M",  "log-max",    "LogFileMaxSize", "log file max size(unit: k/m/g)"],
    log_async : bool   => ["",   "log-async",  "", "enabled async log"],
    no_console: bool   => ["",   "no-console", "", "disabled log to console"],
    host      : String => ["H",  "host",       "HOST", "set dns server listen address"],
    port      : String => ["p",  "port",       "PORT", "set dns server listen port"],
    dns       : String => ["d",  "dns",        "DNS", "set parent dns server address"],
    hosts     : String => ["b",  "hosts",      "HOSTS", "set hosts file path"],
    ttl       : String => ["t",  "ttl",        "TTL", "set dns record ttl seconds"],
    key       : String => ["k",  "key",        "KEY", "set dyndns update key"]
);

impl Default for AppConf {
    fn default() -> Self {
        AppConf {
            log_level  : String::from("info"),
            log_file   : String::new(),
            log_max    : String::from("10m"),
            log_async  : false,
            no_console : false,
            host       : String::from("0.0.0.0"),
            port       : String::from("53"),
            dns        : String::from("0.0.0.0"), // 根服务器a的地址 198.41.0.4
            hosts      : String::new(),
            ttl        : String::from("300"),
            key        : String::new(),
        }
    }
}

fn init() -> bool {
    let version = format!("{APP_NAME} version {APP_VER} CopyLeft Kivensoft 2015-2023.");
    let mut ac = AppConf::default();
    if !appcfg::parse_args(&mut ac, &version).unwrap() {
        return false;
    }
    let ac = ac.init();

    let log_level = asynclog::parse_level(&ac.log_level).unwrap();
    let log_max = asynclog::parse_size(&ac.log_max).unwrap();

    if log_level == log::Level::Trace {
        println!("config setting: {ac:#?}\n");
    }

    asynclog::Builder::new()
        .level(log_level)
        .log_file(ac.log_file.clone())
        .log_file_max(log_max)
        .use_console(!ac.no_console)
        .use_async(ac.log_async)
        .builder()
        .expect("init log failed");

    // 在控制台输出logo
    if let Some((s1, s2)) = G_BANNER.split_once('?') {
        // let s2 = &s2[APP_VER.len() - 1..];
        let baner = format!("{s1}{APP_VER}{s2}");
        appcfg::print_banner(&baner, true);
    }

    true
}

fn main() {
    if !init() { return; }

    let ac = AppConf::get();

    let listen_addr = format!("{}:{}", ac.host, ac.port);
    let ttl: u32 = ac.ttl.parse().unwrap();
    let mut dns_server = DnsServer::create(&listen_addr, &ac.dns, ttl, &ac.key)
        .expect("can't create dns server");

    // 加载hosts file
    if !ac.hosts.is_empty() {
        let mut hosts_config = HostsConfig::new(&ac.hosts)
            .expect("load app config file failed");
        while let Some((host, ip)) = hosts_config.next().expect("load host config failed") {
            dns_server.register_host(host, ip).expect("can't register host");
        }
    }

    dns_server.run(128).unwrap();
}
