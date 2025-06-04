#[macro_export]
macro_rules! fatal {
    ($($arg:tt)+) => {
        log::error!($($arg)+);
        std::process::exit(1);
    };
}
