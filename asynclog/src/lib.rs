use std::{io::{Write, BufWriter, LineWriter}};
use std::sync::{mpsc::Sender, Mutex};
use std::str::FromStr;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

static mut LOG_INITED: bool = false;

/// `Builder` is a struct that holds the configuration for the logger.
///
/// The `level` field is the minimum log level that will be logged. The `log_file` field is the name of
/// the log file. The `log_file_max` field is the maximum size of the log file in megabytes. The
/// `use_console` field is a boolean that indicates whether or not to log to the console. The
/// `use_async` field is a boolean that indicates whether or not to use an asynchronous logger.
///
/// The `new()` method
///
/// Properties:
///
/// * `level`: The log level to use.
/// * `log_file`: The name of the log file.
/// * `log_file_max`: The maximum number of log files to keep.
/// * `use_console`: If true, the logger will log to the console.
/// * `use_async`: Whether to use the async logger or not.
///
/// # Examples
///
/// ```
/// asnyclog::Builder::new()
///     .level(log::LevelFilter::Debug)
///     .log_file(String::from("./app.log"))
///     .log_file_max(1024 * 1024)
///     .use_console(true)
///     .use_async(true)
///     .builder()?;
/// ```
pub struct Builder {
    level: log::LevelFilter,
    log_file: String,
    log_file_max: u32,
    use_console: bool,
    use_async: bool,
}

impl Builder {
    #[inline]
    pub fn new() -> Self {
        Self {
            level: log::LevelFilter::Info,
            log_file: String::new(),
            log_file_max: 10 * 1024 * 1024,
            use_console: true,
            use_async: true
        }
    }

    #[inline]
    pub fn builder(self) -> Result<()> {
        init_log(self.level, self.log_file, self.log_file_max, self.use_console, self.use_async)
    }

    #[inline]
    pub fn level(mut self, level: log::LevelFilter) -> Self {
        self.level = level; self
    }

    #[inline]
    pub fn log_file(mut self, log_file: String) -> Self {
        self.log_file = log_file; self
    }

    #[inline]
    pub fn log_file_max(mut self, log_file_max: u32) -> Self {
        self.log_file_max = log_file_max; self
    }

    #[inline]
    pub fn use_console(mut self, use_console: bool) -> Self {
        self.use_console = use_console; self
    }

    #[inline]
    pub fn use_async(mut self, use_async: bool) -> Self {
        self.use_async = use_async; self
    }
}

/// It creates a new logger, initializes it, and then sets it as the global logger
///
/// Arguments:
///
/// * `level`: log level
/// * `log_file`: The log file path. ignore if the value is empty
/// * `log_file_max`: The maximum size of the log file, The units that can be used are k/m/g.
/// * `use_console`: Whether to output to the console
/// * `use_async`: Whether to use asynchronous logging, if true, the log will be written to the file in
/// a separate thread, and the log will not be blocked.
///
/// Returns:
///
/// A Result<(), anyhow::error>
///
/// # Examples
///
/// ```
/// asnyclog::init_log(log::LevelFilter::Debug, String::from("./app.log", 1024 * 1024, true, true)?;
/// ````
pub fn init_log(level: log::LevelFilter, log_file: String, log_file_max: u32, use_console: bool, use_async: bool) -> Result<()> {
    if unsafe { LOG_INITED } { return Err("init_log must run once!".into()); }
    unsafe { LOG_INITED = true; }

    log::set_max_level(level);

    let logger = Box::new(AsyncLogger {
        level,
        log_file: log_file,
        max_size: log_file_max,
        logger_data: Mutex::new(LogData {
            log_size: 0, console: None, fileout: None, sender: None,
        }),
    });

    let logger = Box::leak(logger);
    let plog = logger as *mut AsyncLogger;
    let mut logger_data = logger.logger_data.lock().expect("init_log call mutex lock error");

    // 如果启用控制台输出，创建一个控制台共享句柄
    if use_console {
        logger_data.console = Some(LineWriter::new(std::io::stdout()));
    }

    // 如果启用文件输出，打开日志文件
    if !logger.log_file.is_empty() {
        let f = std::fs::OpenOptions::new().append(true).create(true).open(&logger.log_file)?;
        logger_data.log_size = std::fs::metadata(&logger.log_file)?.len() as u32;
        logger_data.fileout = Some(LogWriter::new(f));
    }

    // 如果启用异步日志，开启一个线程不停读取channel中的数据进行日志写入，属于多生产者单消费者模式
    if use_async {
        let (sender, receiver) = std::sync::mpsc::channel::<AsyncLogType>();
        logger_data.sender = Some(sender);
        let logger = unsafe { &*plog };
        std::thread::spawn(move || {
            loop {
                match receiver.recv() {
                    Ok(data) => match data {
                        AsyncLogType::Message(msg) => logger.write(msg.as_bytes()),
                        AsyncLogType::Flush => logger.flush_inner(),
                    },
                    Err(e) => {
                        panic!("logger channel recv error: {}", e);
                    }
                }
            }
        });
    }

    // 设置全局日志对象
    log::set_logger(logger).expect("init_log call set_logger error");

    Ok(())
}

/// It takes a string and returns a `Result` of a `log::LevelFilter`
///
/// Arguments:
///
/// * `level`: The log level(off/error/warn/info/debug/trace) to parse.
///
/// Returns:
///
/// A Result<log::LevelFilter>
pub fn parse_level(level: &str) -> Result<log::LevelFilter> {
    match log::LevelFilter::from_str(level) {
        Ok(num) => Ok(num),
        Err(_) => Err("can't parse log level".into()),
    }
}

/// It parses a string into a number, The units that can be used are k/m/g
///
/// Arguments:
///
/// * `size`: The size of the file to be generated(uints: k/m/g).
///
/// Returns:
///
/// A Result<u32, anyhow::Error>
pub fn parse_size(size: &str) -> Result<u32> {
    match size.parse() {
        Ok(n) => Ok(n),
        Err(_) => match size[..size.len() - 1].parse() {
            Ok(n) => {
                let s = size.as_bytes();
                match s[s.len() - 1] {
                    b'b' | b'B' => Ok(n),
                    b'k' | b'K' => Ok(n * 1024),
                    b'm' | b'M' => Ok(n * 1024 * 1024),
                    b'g' | b'G' => Ok(n * 1024 * 1024 * 1024),
                    _ => Err("parse size error, unit is unknown".into()),
                }
            },
            Err(e) => Err(e.into()),
        }
    }
}

enum AsyncLogType {
    Message(String),
    Flush,
}

struct LogData {
    log_size:   u32,                                    // 当前日志文件的大小，跟随写入新的日志内容而变化
    console:    Option<LineWriter<std::io::Stdout>>,    // 控制台对象，如果启用了控制台输出，则对象有值
    fileout:    Option<LogWriter>,                      // 文件对象，如果启用了文件输出，则对象有值
    sender:     Option<Sender<AsyncLogType>>,           // 异步发送频道，如果启用了异步日志模式，则对象有值
}

struct AsyncLogger {
    level:          log::LevelFilter,   // 日志的有效级别，小于该级别的日志允许输出
    log_file:       String,             // 日志文件名
    max_size:       u32,                // 日志文件允许的最大长度
    logger_data:    Mutex<LogData>,     // 日志关联的动态变化的数据
}

impl AsyncLogger {
    // 输出日志到控制台和文件
    fn write(&self, msg: &[u8]) {
        let mut logger_data = self.logger_data.lock().unwrap();

        // 如果启用了控制台输出，则写入控制台
        if let Some(ref mut console) = logger_data.console {
            console.write_all(msg).expect("output log console error");
        }

        let mut curr_size = logger_data.log_size;

        // 判断日志长度是否到达最大限制，如果到了，需要备份当前日志文件并重新创建新的日志文件
        if curr_size > self.max_size {
            let mut log_file_closed = false;

            // 如果启用了日志文件，刷新缓存并关闭日志文件
            if let Some(ref mut fileout) = logger_data.fileout {
                fileout.flush().unwrap();
                drop(fileout);
                log_file_closed = true;
            }

            // 之所以把关闭文件和重新创建文件分开写，是因为rust限制了可变借用(fileout)只允许1次
            if log_file_closed {
                // 删除已有备份，并重命名现有文件为备份文件
                let bak = format!("{}.bak", self.log_file);
                std::fs::remove_file(&bak).unwrap_or_default();

                std::fs::rename(&self.log_file, &bak)
                        .expect("rename log file to backup error");

                let f = std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&self.log_file)
                        .expect("reopen log file error");

                logger_data.fileout = Some(LogWriter::new(f));
                curr_size = 0;
            }
        }

        if let Some(ref mut fileout) = logger_data.fileout {
            fileout.write_all(msg).expect("write log file error");
            logger_data.log_size = curr_size + msg.len() as u32;
        }
    }

    // 刷新日志的控制台和文件缓存
    fn flush_inner(&self) {
        let mut logger_data = self.logger_data.lock().unwrap();

        if let Some(ref mut console) = logger_data.console {
            console.flush().expect("flush log console error");
        }

        if let Some(ref mut fileout) = logger_data.fileout {
            fileout.flush().expect("flush log file error");
        }
    }
}

impl log::Log for AsyncLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool { metadata.level() <= self.level }

    fn log(&self, record: &log::Record) {
        if record.metadata().level() > self.level { return; }

        let now = chrono::Local::now().format("%m-%d %H:%M:%S");

        // 日志条目格式化
        let msg = if self.level >= log::LevelFilter::Debug {
            format!("[\x1b[36m{}\x1b[0m] [{}{:5}\x1b[0m] [{}::{}] - {}\n",
                    now,
                    level_color(record.level()), record.level(),
                    record.target(), record.line().unwrap_or(0),
                    record.args())
        } else {
            format!("[{}] [{:5}] - {}\n", now, record.level(), record.args())
        };

        let logger_data = self.logger_data.lock().unwrap();
        // 采用独立的单线程写入日志的方式，向channel发送要写入的日志消息即可
        if let Some(ref sender) = logger_data.sender {
            sender.send(AsyncLogType::Message(msg)).unwrap();
        } else {
            // 不采用独立写日志线程的情况下，需要先释放锁，因为write函数里面会进行加锁，
            // 如果不释放，则会造成死锁
            drop(logger_data);
            self.write(msg.as_bytes());
        }
    }

    fn flush(&self) {
        let logger_data = self.logger_data.lock().unwrap();
        if let Some(ref sender) = logger_data.sender {
            sender.send(AsyncLogType::Flush).unwrap();
        } else {
            drop(logger_data);
            self.flush_inner();
        }
    }
}

// 日志文件写入对象，提供过滤掉ansi颜色字符的功能
struct LogWriter(BufWriter<std::fs::File>);

impl LogWriter {
    pub fn new(file: std::fs::File) -> Self {
        LogWriter(BufWriter::with_capacity(512, file))
    }
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len();
        let (mut last_pos, mut i) = (0, 0);

        // 过滤ansi颜色
        if len > 3 {
            while i < len - 3 {
                if buf[i] == 0x1b && buf[i + 1] == b'[' {
                    let n = if buf[i + 3] == b'm' { 4 } else { 5 };
                    self.0.write_all(&buf[last_pos .. i])?;
                    i += n;
                    last_pos = i;
                } else {
                    i += 1;
                }
            }
        }

        // 写入剩余的数据
        self.0.write_all(&buf[last_pos .. len])?;

        // 如果已换行符结尾, 则刷新缓冲区
        if len > 0 && buf[len - 1] == b'\n' {
            self.0.flush()?;
        }

        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

// 返回日志级别对应的ansi颜色
fn level_color(level: log::Level) -> &'static str {
    // const RESET:    &str = "\x1b[0m";
    // const BLACK:    &str = "\x1b[30m";
    const RED:      &str = "\x1b[31m";
    const GREEN:    &str = "\x1b[32m";
    const YELLOW:   &str = "\x1b[33m";
    const BLUE:     &str = "\x1b[34m";
    const MAGENTA:  &str = "\x1b[35m";
    // const CYAN:     &str = "\x1b[36m";
    // const WHITE:    &str = "\x1b[37m";

    match level {
        log::Level::Trace => GREEN,
        log::Level::Debug => YELLOW,
        log::Level::Info => BLUE,
        log::Level::Warn => MAGENTA,
        log::Level::Error => RED,
    }
}
