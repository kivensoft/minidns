use std::error::Error;
use getopts::Options;
use simple_config_parser::Config;
use rand::Rng;
use crate::ansi_color::AnsiColor;

const G_BANNER: &str = r##"
              _       _     __ Kivensoft         
   ____ ___  (_)___  (_)___/ /___  _____
  / __ `__ \/ / __ \/ / __  / __ \/ ___/
 / / / / / / / / / / / /_/ / / / (__  ) 
/_/ /_/ /_/_/_/ /_/_/\__,_/_/ /_/____/  
"##;

macro_rules! set_opt_flag {
    ($opts:expr, $short_opt:literal, $long_opt:literal, $opt_name:literal, $desc:literal, bool) => {
        $opts.optflag($short_opt, $long_opt, $desc)
    };
    ($opts:expr, $short_opt:literal, $long_opt:literal, $opt_name:literal, $desc:literal, $_:ty) => {
        $opts.optopt($short_opt, $long_opt, $desc, $opt_name)
    };
}

macro_rules! get_opt_value {
    ($matches:expr, "help", $out_val:expr, $t:ty) => {};
    ($matches:expr, "conf-file", $out_val:expr, $t:ty) => {};
    ($matches:expr, $name:expr, $out_val:expr, String) => {
        if let Some(s) = $matches.opt_str($name) {
            $out_val = s;
        }
    };
    ($matches:expr, $name:expr, $out_val:expr, bool) => {
        if $matches.opt_present($name) {
            $out_val = true;
        }
    };
    ($matches:expr, $name:expr, $out_val:expr, $t:ty) => {
        if let Some(s) = $matches.opt_str($name) {
            match s.parse::<$t>() {
                Ok(n) => $out_val = n,
                Err(_) => return Err(format!("command line option {} is not a number", $name)),
            }
        }
    };
}

macro_rules! get_cfg_value {
    ($cfg: expr, "conf-file", $out_val: expr, $t:ty) => {};
    ($cfg: expr, $name: expr, $out_val: expr, String) => {
        if let Ok(s) = $cfg.get_str($name) {
            $out_val = s;
        }
    };
    ($cfg: expr, $name: expr, $out_val: expr, bool) => {
        if let Ok(s) = $cfg.get_str($name) {
            $out_val = s.to_lowercase() == "true";
        }
    };
    ($cfg: expr, $name: expr, $out_val: expr, $t:ty) => {
        if let Ok(s) = $cfg.get_str($name) {
            match s.parse::<$t>() {
                Ok(n) => $out_val = n,
                Err(_) => return Err(format!("cconfigure file param {} is not a number", $name)),
            }
        }
    };
}

macro_rules! struct_define {
    ( $struct_name:ident, $( $field:ident : $type:tt =>
            [ $short_opt:literal, $long_opt:tt, $opt_name:literal, $desc:literal ]),+ ) => {
        
        #[derive(Debug)]
        pub struct $struct_name {
            $( pub $field: $type,)*
        }

        impl $struct_name {
            // $( #[allow(dead_code)]fn $field() -> &'static str { $long_opt } )*

            fn to_opts() -> getopts::Options {
                let mut opts = getopts::Options::new();
                $( set_opt_flag!(opts, $short_opt, $long_opt, $opt_name, $desc, $type); )*
                opts
            }

            fn set_from_getopts(&mut self, matches: &getopts::Matches) -> Result<(), String> {
                $( get_opt_value!(matches, $long_opt, self.$field, $type); )*
                Ok(())
            }

            fn set_from_cfg(&mut self, cfg: &simple_config_parser::Config) -> Result<(), String> {
                $( get_cfg_value!(cfg, $long_opt, self.$field, $type); )*
                Ok(())
            }
        }
    };
}

struct_define!(AppConf,
    help      : bool   => ["",   "help", "", "print this help menu"],
    conf_file : String => ["c",  "conf-file", "CONF_FILE", "set configure file path"],
    log_level : String => ["L",  "log-level", "LOG_LEVEL", "set log level(trace/debug/info/warn/error/off)"],
    log_file  : String => ["F",  "log-file", "LOG_FILE", "set log file path"],
    host      : String => ["h",  "host", "HOST", "set dns server listen address"],
    port      : u16    => ["p",  "port", "PORT", "set dns server listen port"],
    dns       : String => ["d",  "dns", "DNS", "set parent dns server address"],
    hosts_file: String => ["b",  "hosts-file",  "HOSTS_FILE", "set hosts file path"],
    ttl       : u32    => ["t",  "ttl", "TTL", "set dns record ttl seconds"],
    key       : String => ["k",  "key", "KEY", "set dyndns update key"]
);

impl Default for AppConf {
    fn default() -> Self {
        AppConf {
            help       : false,
            conf_file  : String::new(),
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

fn print_usage(prog: &str, opts: &Options) {
    let path = std::path::Path::new(prog);
    let prog = path.file_name().unwrap().to_str().unwrap();
    let brief = format!("Usage: {} [options]", prog);

    print_banner(G_BANNER);
    println!("{}", opts.usage(&brief));
}

pub fn print_banner(banner: &str) {
    let mut rng = rand::thread_rng();
    let mut lines = String::new();
    let c_reset: &str = &AnsiColor::Z.to_string();

    for line in banner.split('\n') {
        let c = AnsiColor::from(rng.gen_range(2..9));
        lines.push_str(&c.to_string());
        lines.push_str(line);
        lines.push_str(c_reset);
        lines.push('\n');
    }

    println!("{}", lines);
}

/// 解析命令行参数
/// 
/// Returns: 成功: Ok(ac), 显示帮助并退出: Ok(None), 错误 Err(e)
/// 
/// a Result<Option<AppConf>, Box<dyn Error>>.
pub fn parse_args() -> Result<Option<AppConf>, Box<dyn Error>> {
    let mut ac = AppConf::default();
    let mut args = std::env::args();
    let prog = args.next().unwrap();

    let opts = AppConf::to_opts();

    let matches = match opts.parse(args) {
        Ok(m) => m,
        Err(_) => {
            print_usage(&prog, &opts);
            return Ok(None);
        },
    };

    if matches.opt_present("help") {
        print_usage(&prog, &opts);
        return Ok(None);
    }

    ac.conf_file = match matches.opt_str("conf-file") {
        Some(s) => s,
        None => {
            let mut path = std::path::PathBuf::from(prog);
            path.set_extension("conf");
            path.to_str().ok_or("program name error")?.to_owned()
        }
    };
    if let Ok(cfg) = Config::new().file(&ac.conf_file) {
        ac.set_from_cfg(&cfg)?;
    }

    ac.set_from_getopts(&matches)?;

    print_banner(G_BANNER);

    Ok(Some(ac))
}

