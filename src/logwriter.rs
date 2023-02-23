use std::{fs::File, io::Write, io::BufWriter};

pub struct LogWriter(BufWriter<File>);

impl LogWriter {
    pub fn new(file: File) -> LogWriter {
        LogWriter(BufWriter::with_capacity(512, file))
    }
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len();
        let mut last_pos = 0;
        let mut i = 0;
        
        // 过滤ansi颜色
        if len > 3 {
            while i < len - 3 {
                if buf[i] == 0x1b && buf[i + 1] == b'[' {
                    let n = if buf[i + 3] == b'm' { 4 } else { 5 };
                    self.0.write(&buf[last_pos .. i])?;
                    i += n;
                    last_pos = i;
                } else {
                    i += 1;
                }
            }
        }

        // 写入剩余的数据
        self.0.write(&buf[last_pos .. len])?;

        // 如果已换行符结尾, 则刷新缓冲区
        if len > 0 {
            match buf[len - 1] {
                b'\n' | b'\r' => self.0.flush()?,
                _ => {},
            }
        }

        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
