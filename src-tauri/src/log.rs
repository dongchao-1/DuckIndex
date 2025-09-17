use log::LevelFilter;
use log4rs::{
    append::rolling_file::policy::compound::{
        roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use std::env;

use crate::dirs::get_log_dir;

pub fn init_logger() {
    let level_filter;
    if let Ok(log_level) = env::var("DUCKINDEX_LOG_LEVEL") {
        level_filter = match log_level.to_lowercase().as_str() {
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,
            "debug" => LevelFilter::Debug,
            "trace" => LevelFilter::Trace,
            _ => {
                panic!("未知的日志级别: {log_level}");
            }
        }
    } else {
        level_filter = LevelFilter::Info;
    }

    let trigger = SizeTrigger::new(64 * 1024 * 1024);
    let roller = FixedWindowRoller::builder()
        .build(
            get_log_dir().join("duckindex_{}.log.gz").to_str().unwrap(),
            7,
        )
        .unwrap();

    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    let pattern = "{d(%Y-%m-%d %H:%M:%S%.3f)} {T} {f}:{L} [{l}] {m}{n}";
    let appender = if env::var("DUCKINDEX_TEST_DIR").is_ok() {
        Box::new(
            log4rs::append::console::ConsoleAppender::builder()
                .encoder(Box::new(PatternEncoder::new(pattern)))
                .build(),
        ) as Box<dyn log4rs::append::Append>
    } else {
        Box::new(
            log4rs::append::rolling_file::RollingFileAppender::builder()
                .encoder(Box::new(PatternEncoder::new(pattern)))
                .build(get_log_dir().join("duckindex.log"), Box::new(policy))
                .unwrap(),
        ) as Box<dyn log4rs::append::Append>
    };

    let log_config = Config::builder()
        .appender(Appender::builder().build("appender", appender))
        // 为 lopdf 设置 error级别
        .logger(
            log4rs::config::Logger::builder()
                .appender("appender")
                .additive(false)
                .build("lopdf", LevelFilter::Error),
        )
        // Root logger 为你的应用设置环境变量指定的级别
        .build(Root::builder().appender("appender").build(level_filter))
        .unwrap();

    log4rs::init_config(log_config).unwrap();
}

#[cfg(test)]
mod tests {
    use crate::test::test_mod::TestEnv;
    use log::{debug, error, info, trace, warn};

    #[test]
    fn test_init_logger() {
        let _env = TestEnv::new();

        error!("error log.");
        warn!("warn log.");
        info!("info log.");
        debug!("debug log.");
        trace!("trace log.");
    }
}
