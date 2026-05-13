use crate::errors::{AsterError, thumbnail_generation_error_with_subcode};

pub(super) fn thumbnail_render_failed(message: impl Into<String>) -> AsterError {
    thumbnail_generation_error_with_subcode("thumbnail.render_failed", message)
}

pub(super) fn thumbnail_output_invalid(message: impl Into<String>) -> AsterError {
    thumbnail_generation_error_with_subcode("thumbnail.output_invalid", message)
}
