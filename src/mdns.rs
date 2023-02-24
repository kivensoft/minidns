#[macro_use(anyhow, bail)] extern crate anyhow;
#[macro_use] mod ansicolor;
#[macro_use] mod appconf;

mod logwriter;
mod bufutil;
mod dnsutil;
mod dnsserver;
mod hostsconf;

use dnsserver::*;
use hostsconf::*;

use std::str::FromStr;
use anyhow::{Result, Context};

const G_BANNER: &str = r##"
              _       _     __ Kivensoft         
   ____ ___  (_)___  (_)___/ /___  _____
  / __ `__ \/ / __ \/ / __  / __ \/ ___/
 / / / / / / / / / / / /_/ / / / (__  ) 
/_/ /_/ /_/_/_/ /_/_/\__,_/_/ /_/____/  
"##;

appconfig_define!(AppConf,
    log_level : String => ["L",  "log-level", "LOG_LEVEL", "set log level(trace/debug/info/warn/error/off)"],
    log_file  : String => ["F",  "log-file", "LOG_FILE", "set log file path"],
    host      : String => ["H",  "host", "HOST", "set dns server listen address"],
    port      : u16    => ["p",  "port", "PORT", "set dns server listen port"],
    dns       : String => ["d",  "dns", "DNS", "set parent dns server address"],
    hosts_file: String => ["b",  "hosts-file",  "HOSTS_FILE", "set hosts file path"],
    ttl       : u32    => ["t",  "ttl", "TTL", "set dns record ttl seconds"],
    key       : String => ["k",  "key", "KEY", "set dyndns update key"]
);

pub fn init_log(log_level: &str, log_file: &str) -> Result<()> {
    let level = simplelog::LevelFilter::from_str(log_level).with_context(|| "log level format error")?;

    let mut cfg = simplelog::ConfigBuilder::new();
    cfg.set_level_padding(simplelog::LevelPadding::Right)
        .set_time_format_custom(simplelog::format_description!("[[[month]-[day] [hour]:[minute]:[second]]"))
        .set_time_offset_to_local().unwrap();
    let cfg = cfg.build();

    let mut vec: Vec<Box<dyn simplelog::SharedLogger>> = Vec::new();
    vec.push(simplelog::TermLogger::new(level, cfg.clone(), simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto));

    // 默认只记录到控制台, 除非log_file不为空
    if log_file != "" {
        let log_file = std::fs::OpenOptions::new().append(true).create(true).open(log_file)
                .with_context(|| format!("open {log_file} failed"))?;
        vec.push(simplelog::WriteLogger::new(level, cfg, logwriter::LogWriter::new(log_file)));
    }

    simplelog::CombinedLogger::init(vec).with_context(|| "init simplelog failed")?;

    Ok(())
}

impl Default for AppConf {
    fn default() -> Self {
        AppConf {
            log_level  : String::from("info"),
            log_file   : String::new(),
            host       : String::from("0.0.0.0"),
            port       : 53,
            dns        : String::from("0.0.0.0"), // 根服务器a的地址 198.41.0.4
            hosts_file : String::new(),
            ttl        : 300,
            key        : String::new(),
        }
    }
}

fn init() -> Result<Option<Box<DnsServer>>> {
    let mut ac = AppConf::default();
    if appconf::parse_args(&mut ac, G_BANNER)? {
        if &ac.log_level.to_lowercase() == "trace" {
            println!("config setting: {ac:#?}\n");
        }

        init_log(&ac.log_level, &ac.log_file).with_context(|| "init log failed")?;

        let listen_addr = format!("{}:{}", ac.host, ac.port);
        let mut dns_server = Box::new(DnsServer::create(&listen_addr, &ac.dns, ac.ttl, &ac.key)?);

        // 加载hosts file
        if !ac.hosts_file.is_empty() {
            let mut hosts_config = HostsConfig::new(&ac.hosts_file).with_context(|| "load app config file failed")?;
            while let Some((host, ip)) = hosts_config.next().with_context(|| "load host config failed")? {
                dns_server.register_host(host, ip)?;
            }
        }

        return Ok(Some(dns_server));
    }
    Ok(None)
}

fn main() -> Result<()> {
    if let Some(mut dns_server) = init()? {
        dns_server.run(128)?;
    }
    Ok(())
}
