#[allow(dead_code)] pub const Z: &str = "\x1b[0m";
#[allow(dead_code)] pub const K: &str = "\x1b[30m";
#[allow(dead_code)] pub const R: &str = "\x1b[31m";
#[allow(dead_code)] pub const G: &str = "\x1b[32m";
#[allow(dead_code)] pub const Y: &str = "\x1b[33m";
#[allow(dead_code)] pub const B: &str = "\x1b[34m";
#[allow(dead_code)] pub const M: &str = "\x1b[35m";
#[allow(dead_code)] pub const C: &str = "\x1b[36m";
#[allow(dead_code)] pub const W: &str = "\x1b[37m";

#[macro_export] macro_rules! ansi_color_format {
    ($c:expr, $($t:tt)*) => {
        format_args!("{}{}\x1b[0m", $c, format_args!($($t)*))
    };
}

/// example:
/// ```
/// println!("this is {}", ac_red!("red"));
/// println!("this is {}", ac_red!("my name is {}", "kiven"));
/// ```
#[macro_export] macro_rules! ac_black { ($($t:tt)*) => { ansi_color_format!($crate::ansi_color::K, $($t)*) }; }
#[macro_export] macro_rules! ac_red { ($($t:tt)*) => { ansi_color_format!($crate::ansi_color::R, $($t)*) }; }
#[macro_export] macro_rules! ac_green { ($($t:tt)*) => { ansi_color_format!($crate::ansi_color::G, $($t)*) }; }
#[macro_export] macro_rules! ac_yellow { ($($t:tt)*) => { ansi_color_format!($crate::ansi_color::Y, $($t)*) }; }
#[macro_export] macro_rules! ac_blue { ($($t:tt)*) => { ansi_color_format!($crate::ansi_color::B, $($t)*) }; }
#[macro_export] macro_rules! ac_magenta { ($($t:tt)*) => { ansi_color_format!($crate::ansi_color::M, $($t)*) }; }
#[macro_export] macro_rules! ac_cyan { ($($t:tt)*) => { ansi_color_format!($crate::ansi_color::C, $($t)*) }; }
#[macro_export] macro_rules! ac_white { ($($t:tt)*) => { ansi_color_format!($crate::ansi_color::W, $($t)*) }; }

pub enum AnsiColor { Z, K, R, G, Y, B, M, C, W }

impl std::fmt::Display for AnsiColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AnsiColor::Z => "\x1b[0m",      // 重置, reset
            AnsiColor::K => "\x1b[30m",     // 黑, black
            AnsiColor::R => "\x1b[31m",     // 红, red
            AnsiColor::G => "\x1b[32m",     // 绿, green
            AnsiColor::Y => "\x1b[33m",     // 黄, yellow
            AnsiColor::B => "\x1b[34m",     // 蓝, blue
            AnsiColor::M => "\x1b[35m",     // 紫, magenta
            AnsiColor::C => "\x1b[36m",     // 青, cyan
            AnsiColor::W => "\x1b[37m",     // 白, white
        };
        write!(f, "{}", s)
    }
}

impl std::convert::From<u32> for AnsiColor {
    fn from(value: u32) -> Self {
        match value {
            0 => AnsiColor::Z,
            1 => AnsiColor::K,
            2 => AnsiColor::R,
            3 => AnsiColor::G,
            4 => AnsiColor::Y,
            5 => AnsiColor::B,
            6 => AnsiColor::M,
            7 => AnsiColor::C,
            8 => AnsiColor::W,
            _ => AnsiColor::Z,
        }
    }
}
