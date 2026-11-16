use crate::*;

/// Check annotated variable declarations and assignments against their runtime values.
/// Disabled by default; inferred variable types are never enforced by this option.
pub static ENFORCE_RUNTIME_TYPE_ANNOTATIONS: ExperimentalOption =
    ExperimentalOption::new(&EnforceRuntimeTypeAnnotations);

struct EnforceRuntimeTypeAnnotations;
impl ExperimentalOptionMarker for EnforceRuntimeTypeAnnotations {
    const IDENTIFIER: &'static str = "enforce-runtime-type-annotations";
    const DESCRIPTION: &'static str = "Enforce annotated variable types at runtime.";
    const STATUS: Status = Status::OptIn;
}
