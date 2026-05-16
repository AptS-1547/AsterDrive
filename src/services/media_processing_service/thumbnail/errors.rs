use crate::api::subcode::ApiSubcode;
use crate::errors::{AsterError, thumbnail_generation_error_with_subcode};

pub(super) fn thumbnail_render_failed(message: impl Into<String>) -> AsterError {
    thumbnail_generation_error_with_subcode(ApiSubcode::ThumbnailRenderFailed, message)
}

pub(super) fn thumbnail_output_invalid(message: impl Into<String>) -> AsterError {
    thumbnail_generation_error_with_subcode(ApiSubcode::ThumbnailOutputInvalid, message)
}
