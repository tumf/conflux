use tracing_subscriber::filter::LevelFilter;

pub const STDOUT_LOG_LEVEL_FILTER: LevelFilter = LevelFilter::INFO;

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;

    #[test]
    fn stdout_filter_allows_info_and_suppresses_debug_trace() {
        assert!(STDOUT_LOG_LEVEL_FILTER >= Level::INFO);
        assert!(STDOUT_LOG_LEVEL_FILTER < Level::DEBUG);
        assert!(STDOUT_LOG_LEVEL_FILTER < Level::TRACE);
    }
}
