use anyhow::Result;

pub struct BytePacketBuffer {
    pub buf: [u8; 512],
    pub pos: usize,
    pub len: usize,
}

impl BytePacketBuffer {
    pub fn new() -> BytePacketBuffer {
        BytePacketBuffer {
            buf: [0; 512],
            pos: 0,
            len: 512,
        }
    }

    fn check_range(&self, pos: usize) -> Result<()> {
        if pos >= self.len {
            bail!("End of buffer");
        }
        Ok(())
    }

    pub fn pos(&self) -> usize { self.pos }

    pub fn step(&mut self, steps: usize) -> Result<()> {
        self.pos += steps;
        Ok(())
    }

    pub fn seek(&mut self, pos: usize) -> Result<()> {
        self.pos = pos;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn read(&mut self) -> Result<u8> {
        self.check_range(self.pos)?;
        let res = self.buf[self.pos];
        self.pos += 1;
        Ok(res)
    }

    pub fn get(&mut self, pos: usize) -> Result<u8> {
        self.check_range(pos)?;
        Ok(self.buf[pos])
    }

    pub fn get_range(&mut self, start: usize, len: usize) -> Result<&[u8]> {
        self.check_range(start + len - 1)?;
        Ok(&self.buf[start .. start + len])
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        self.check_range(self.pos + 1)?;
        let res = ((self.buf[self.pos] as u16) << 8) | (self.buf[self.pos + 1] as u16);
        self.pos += 2;
        Ok(res)
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        self.check_range(self.pos + 3)?;
        let res = ((self.buf[self.pos] as u32) << 24)
            | ((self.buf[self.pos + 1] as u32) << 16)
            | ((self.buf[self.pos + 2] as u32) << 8)
            | ((self.buf[self.pos + 3] as u32) << 0);
        self.pos += 4;
        Ok(res)
    }

    pub fn read_qname(&mut self, outstr: &mut String) -> Result<()> {
        let mut pos = self.pos();
        let mut jumped = false;

        let mut first = true;
        let delim = ".";
        let max_jumps = 5;
        let mut jumps_performed = 0;
        loop {
            // Dns数据包是不受信任的数据，因此我们需要警惕。某人可以用跳转指令中的循环来制作数据包。这个守卫针对这样的分组
            if jumps_performed > max_jumps {
                bail!("Limit of {max_jumps} jumps exceeded");
            }

            let len = self.get(pos)?;

            // 最高2位为1(0xC0), 表示这不是长度, 而是一个跳转地址
            if (len & 0xC0) == 0xC0 {
                // 首次跳转, 需要更新读取位置, 后续跳转不再更新
                if !jumped {
                    self.seek(pos + 2)?;
                }

                // 跳转地址用2个字节表示
                let b2 = self.get(pos + 1)? as u16;
                let offset = (((len as u16) ^ 0xC0) << 8) | b2;
                pos = offset as usize;
                jumped = true;
                jumps_performed += 1;
                continue;
            }

            pos += 1;

            // 长度为0时,表示字符串终止
            if len == 0 {
                break;
            }

            if first { first = false; }
            else { outstr.push_str(delim); }

            let str_buffer = self.get_range(pos, len as usize)?;
            outstr.push_str(&String::from_utf8_lossy(str_buffer).to_lowercase());

            pos += len as usize;
        }

        if !jumped {
            self.seek(pos)?;
        }

        Ok(())
    }

    pub fn write(&mut self, val: u8) -> Result<()> {
        self.check_range(self.pos)?;
        self.buf[self.pos] = val;
        self.pos += 1;
        Ok(())
    }

    pub fn write_u16(&mut self, val: u16) -> Result<()> {
        self.check_range(self.pos + 1)?;
        self.buf[self.pos] = (val >> 8) as u8;
        self.buf[self.pos + 1] = val as u8;
        self.pos += 2;
        Ok(())
    }

    pub fn write_u32(&mut self, val: u32) -> Result<()> {
        self.check_range(self.pos + 3)?;
        self.buf[self.pos] = (val >> 24) as u8;
        self.buf[self.pos + 1] = (val >> 16) as u8;
        self.buf[self.pos + 2] = (val >> 8) as u8;
        self.buf[self.pos + 3] = val as u8;
        self.pos += 4;
        Ok(())
    }

    pub fn write_qname(&mut self, qname: &str) -> Result<()> {
        self.check_range(self.pos + qname.len())?;

        let mut pos = self.pos;
        for label in qname.split('.') {
            let len = label.len();
            if len > 0x34 {
                bail!("Single label exceeds 63 characters of length");
            }

            self.buf[pos] = len as u8;
            pos += 1;
            for b in label.as_bytes() {
                self.buf[pos] = *b;
                pos += 1;
            }
        }

        self.buf[pos] = 0;
        self.pos = pos + 1;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn set(&mut self, pos: usize, val: u8) -> Result<()> {
        self.check_range(pos)?;
        self.buf[pos] = val;
        Ok(())
    }

    pub fn set_u16(&mut self, pos: usize, val: u16) -> Result<()> {
        self.check_range(pos + 1)?;
        self.buf[pos] = (val >> 8) as u8;
        self.buf[pos + 1] = val as u8;
        Ok(())
    }
}

