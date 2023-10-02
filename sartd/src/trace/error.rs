
pub(crate) trait TraceableError: std::error::Error {
	fn metric_label(&self) -> String;
}
