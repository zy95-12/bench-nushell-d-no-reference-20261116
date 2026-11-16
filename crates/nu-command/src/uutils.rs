use nu_protocol::{PipelineData, ShellError, Span, shell_error::io::IoError};

/// uucore 0.2 has a once-initialized, per-thread localizer for a single utility.
/// A shell can run many utilities on one thread, so isolate each invocation and
/// format its errors there too. This also keeps concurrent pipelines independent.
pub(crate) fn with_locale(
    utility: &str,
    span: Span,
    operation: impl FnOnce() -> Result<PipelineData, ShellError> + Send,
) -> Result<PipelineData, ShellError> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name(format!("uutils {utility}"))
            .spawn_scoped(scope, || {
                uucore::locale::setup_localization(utility).map_err(|error| {
                    ShellError::GenericError {
                        error: "Could not initialize coreutils messages".into(),
                        msg: error.to_string(),
                        span: Some(span),
                        help: None,
                        inner: vec![],
                    }
                })?;
                operation()
            })
            .map_err(|error| ShellError::Io(IoError::new(error, span, None)))?;
        worker
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}
