#[macro_use(anyhow, bail)]
extern crate anyhow;

mod logutils;
mod appconf;
mod bufutil;
mod dnsutil;
mod dnsserver;
mod ansi_color;
mod hostsconf;

use dnsserver::*;
use hostsconf::*;

use anyhow::{Result, Context};

fn init() -> Result<Option<Box<DnsServer>>> {
    if let Some(ac) = appconf::parse_args()? {
        if ac.log_level == "trace" {
            println!("config setting: {ac:#?}\n");
        }

        logutils::init_log(&ac.log_level, &ac.log_file).with_context(|| "init log failed")?;

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
        log::debug!("this is debug");
        dns_server.run(128)?;
    }
    Ok(())
}
