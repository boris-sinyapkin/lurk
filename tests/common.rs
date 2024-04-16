use log::LevelFilter;
use log4rs_test_utils::test_logging::init_logging_once_for;

pub fn init_logging() {
    init_logging_once_for(
        vec!["lurk"],
        LevelFilter::Debug,
        "{h({({l}):5.5})} [{M}] {f}:{L}: {m}{n}",
    );
}