//! CSRF 中间件模块入口。

mod constants;
mod source;
mod token;

#[cfg(test)]
mod tests;

pub use constants::{CSRF_COOKIE, CSRF_HEADER};
pub use source::{
    RequestSourceMode, ensure_request_source_allowed, ensure_service_request_source_allowed,
    is_unsafe_method,
};
pub use token::{build_csrf_token, ensure_double_submit_token, ensure_service_double_submit_token};
