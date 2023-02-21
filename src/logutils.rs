use std::str::FromStr;
use anyhow::{Result, Context};

pub fn init_log(log_level: &str, log_file: &str) -> Result<()> {
    let level = simplelog::LevelFilter::from_str(log_level)
            .with_context(|| "log level format error")?;

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
        vec.push(simplelog::WriteLogger::new(level, cfg, log_file));
    }

    simplelog::CombinedLogger::init(vec).with_context(|| "init simplelog failed")?;

    Ok(())
}