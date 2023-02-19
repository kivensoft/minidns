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
