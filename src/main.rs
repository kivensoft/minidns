mod appconf;
mod bufutil;
mod dnsutil;
mod dnsserver;
mod ansi_color;
mod hostsconf;

use std::str::FromStr;

use bufutil::*;
use dnsserver::*;
use hostsconf::*;

fn init_log(log_level: &str, log_file: &str) -> Result<()> {
    let level = simplelog::LevelFilter::from_str(log_level)?;

    let mut cfg = simplelog::ConfigBuilder::new();
    cfg.set_level_padding(simplelog::LevelPadding::Right)
        .set_time_format_custom(simplelog::format_description!("[[[month]-[day] [hour]:[minute]:[second]]"))
        .set_time_offset_to_local().unwrap();
    let cfg = cfg.build();

    let mut vec: Vec<Box<dyn simplelog::SharedLogger>> = Vec::new();
    vec.push(simplelog::TermLogger::new(level, cfg.clone(), simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto));

    // 默认只记录到控制台, 除非log_file不为空
    if log_file != "" {
        let log_file = std::fs::OpenOptions::new().append(true).create(true).open(log_file)?;
        vec.push(simplelog::WriteLogger::new(level, cfg, log_file));
    }

    simplelog::CombinedLogger::init(vec)?;

    Ok(())
}

fn init() -> Result<Option<Box<DnsServer>>> {
    if let Some(ac) = appconf::parse_args()? {
        if ac.log_level == "trace" {
            println!("config setting: {ac:#?}\n");
        }
        
        init_log(&ac.log_level, &ac.log_file)?;

        let listen_addr = format!("{}:{}", ac.host, ac.port);
        let mut dns_server = Box::new(DnsServer::create(&listen_addr, &ac.dns, ac.ttl, &ac.key)?);

        // 加载hosts file
        if !ac.hosts_file.is_empty() {
            let mut hosts_config = HostsConfig::new(&ac.hosts_file)?;
            while let Some((host, ip)) = hosts_config.next()? {
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
