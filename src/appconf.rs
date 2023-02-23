use anyhow::{Result, Context};
use simple_config_parser::Config;
use crate::ansicolor::AnsiColor;

pub trait AppConfig {
    fn to_opts(&self) -> getopts::Options;
    fn set_from_getopts(&mut self, matches: &getopts::Matches) -> Result<()>;
    fn set_from_cfg(&mut self, cfg: &simple_config_parser::Config) -> Result<()>;
    fn set_from_env(&mut self) -> Result<()>;
}

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
            $out_val = s.parse::<$t>().with_context(
                || format!("program argument {} is not a numbe", $name))?;
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
            $out_val = s.parse::<$t>().with_context(
                || format!("app config file key {} is not a number", $name))?;
        }
    };
}

macro_rules! get_env_value {
    ($name: expr, $out_val: expr, String) => {
        if let Ok(s) = std::env::var($name) {
            $out_val = s;
        }
    };
    ($name: expr, $out_val: expr, bool) => {
        if let Ok(s) = std::env::var($name) {
            $out_val = s.to_lowercase() == "true";
        }
    };
    ($name: expr, $out_val: expr, $t:ty) => {
        if let Ok(s) = std::env::var($name) {
            $out_val = s.parse::<$t>().with_context(
                || format!("environment var {} is not a number", $name))?;
        }
    };
}

macro_rules! appconfig_define {
    ( $struct_name:ident, $( $field:ident : $type:tt =>
            [ $short_opt:literal, $long_opt:tt, $opt_name:literal, $desc:literal ]),+ ) => {
        
        #[derive(Debug)]
        pub struct $struct_name {
            $( pub $field: $type,)*
        }

        impl $crate::appconf::AppConfig for $struct_name {
            fn to_opts(&self) -> getopts::Options {
                let mut opts = getopts::Options::new();
                $( set_opt_flag!(opts, $short_opt, $long_opt, $opt_name, $desc, $type); )*
                opts
            }

            fn set_from_getopts(&mut self, matches: &getopts::Matches) -> Result<()> {
                $( get_opt_value!(matches, $long_opt, self.$field, $type); )*
                Ok(())
            }

            fn set_from_cfg(&mut self, cfg: &simple_config_parser::Config) -> Result<()> {
                $( get_cfg_value!(cfg, $long_opt, self.$field, $type); )*
                Ok(())
            }

            fn set_from_env(&mut self) -> Result<()> {
                $( get_env_value!($long_opt, self.$field, $type); )*
                Ok(())
            }
        }
    };
}

fn print_usage(prog: &str, opts: &getopts::Options, banner: &str) {
    let path = std::path::Path::new(prog);
    let prog = path.file_name().unwrap().to_str().unwrap();
    let brief = format!("Usage: {} {}", ac_cyan!(&prog), ac_green!("[options]"));

    print_banner(banner);
    println!("{}", opts.usage(&brief));
}

pub fn print_banner(banner: &str) {
    use rand::Rng;

    if banner.len() == 0 { return; }

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
/// 使用系统环境变量, 配置文件和命令行参数进行解析并填充ac结构
/// 
/// Returns: 参见 `parse_args_ext`
/// 
#[allow(dead_code)]
pub fn parse_args<T>(ac: &mut T, banner: &str) -> Result<bool>
        where T: AppConfig {
    parse_args_ext(ac, banner, |_| true)
}

/// 解析命令行参数
/// 
/// 使用系统环境变量, 配置文件和命令行参数进行解析并填充ac结构,
/// 并调用f函数判断参数是否合法(返回true合法, 返回false, 打印帮助信息并退出)
/// 
/// Returns: 成功: Ok(ac), 显示帮助并退出: Ok(None), 错误 Err(e)
/// 
pub fn parse_args_ext<T, F>(ac: &mut T, banner: &str, f: F) -> Result<bool>
        where T: AppConfig, F: Fn(&T) -> bool {
    const C_HELP: &str = "help";
    const C_CONF_FILE: &str = "conf-file";

    let mut args = std::env::args();
    let prog = args.next().unwrap();

    let mut opts = ac.to_opts();
    opts.optflag("h", C_HELP, "print this help menu");
    opts.optopt("c",  C_CONF_FILE, "set program config file path", "CONF_FILE");

    let matches = match opts.parse(args).with_context(|| "parse program arguments failed") {
        Ok(m) => m,
        Err(e) => {
            print_usage(&prog, &opts, banner);
            return Err(e);
        },
    };

    if matches.opt_present(C_HELP) {
        print_usage(&prog, &opts, banner);
        return Ok(false);
    }

    // 参数设置优先级：命令行参数 > 配置文件参数 > 环境变量参数
    // 因此, 先读环境变量覆盖缺省参数,然后读配置文件覆盖, 最后用命令行参数覆盖
    ac.set_from_env()?;

    // 从配置文件读取参数, 如果环境变量及命令行未提供配置文件参数, 则允许读取失败, 否则, 读取失败返回错误
    let mut conf_is_set = false;
    let mut conf_file = String::new();
    if let Ok(cf) = std::env::var(C_CONF_FILE) {
        conf_is_set = true;
        conf_file = cf;
    }
    if let Some(cf) = matches.opt_str(C_CONF_FILE) {
        conf_is_set = true;
        conf_file = cf;
    }
    if !conf_is_set {
        let mut path = std::path::PathBuf::from(&prog);
        path.set_extension("conf");
        conf_file = path.to_str().ok_or(anyhow!("program name error"))?.to_owned();
    }
    match Config::new().file(&conf_file) {
        Ok(cfg) => ac.set_from_cfg(&cfg)?,
        Err(_) => {
            if conf_is_set {
                bail!("can't read app config file {conf_file}");
            }
        },
    }

    // 从命令行读取参数
    ac.set_from_getopts(&matches)?;

    if !f(ac) {
        print_usage(&prog, &opts, banner);
        return Ok(false);
    }

    print_banner(banner);

    Ok(true)
}

