use aya::Ebpf;
use aya_log::EbpfLogger;

use crate::Res;

#[derive(Default)]
pub struct EbpfLogs {
    loggers: Vec<EbpfLogger<LogPrinter>>,
}

impl EbpfLogs {
    pub fn attach(&mut self, bpf: &mut Ebpf) -> Res<()> {
        match EbpfLogger::init_with_logger(bpf, LogPrinter) {
            Ok(logger) => self.loggers.push(logger),
            Err(aya_log::Error::MapNotFound) => {}
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }

    pub fn flush(&mut self) {
        for logger in &mut self.loggers {
            logger.flush();
        }
    }
}

struct LogPrinter;

impl log::Log for LogPrinter {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let level = match record.level() {
            log::Level::Error => "\x1b[31mERROR\x1b[0m",
            log::Level::Warn => "\x1b[33mWARN\x1b[0m",
            log::Level::Info => "\x1b[32mINFO\x1b[0m",
            log::Level::Debug => "\x1b[34mDEBUG\x1b[0m",
            log::Level::Trace => "\x1b[35mTRACE\x1b[0m",
        };
        let file = record.file().unwrap_or("?");
        let line = record.line().unwrap_or(0);
        println!("[{level}] {file}:{line}: {}", record.args());
    }

    fn flush(&self) {}
}
