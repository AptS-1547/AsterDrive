use std::borrow::Cow;

use crate::errors::{AsterError, Result};
use validator::{Validate, ValidationError, ValidationErrors};

pub(crate) fn validate_request<T: Validate>(value: &T) -> Result<()> {
    value.validate().map_err(validation_errors_to_aster)
}

pub(crate) fn validate_name(value: &str) -> std::result::Result<(), ValidationError> {
    crate::utils::validate_name(value).map_err(aster_to_validation_error)
}

fn aster_to_validation_error(error: AsterError) -> ValidationError {
    let mut validation_error = ValidationError::new("invalid");
    validation_error.message = Some(Cow::Owned(error.message().to_string()));
    validation_error
}

fn validation_errors_to_aster(errors: ValidationErrors) -> AsterError {
    let mut messages: Vec<String> = errors
        .field_errors()
        .iter()
        .flat_map(|(field, errors)| {
            errors
                .iter()
                .map(move |error| validation_error_message(field, error))
        })
        .collect();
    messages.sort();
    AsterError::validation_error(messages.join(", "))
}

fn validation_error_message(field: &str, error: &ValidationError) -> String {
    error
        .message
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("invalid field '{field}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(validator::Validate)]
    struct NameReq {
        #[validate(custom(function = "crate::api::dto::validation::validate_name"))]
        name: String,
    }

    #[derive(validator::Validate)]
    struct SizeReq {
        #[validate(range(min = 1, message = "total_size must be positive"))]
        total_size: i64,
    }

    #[test]
    fn validate_request_uses_existing_name_rules() {
        let err = validate_request(&NameReq {
            name: "bad/name".to_string(),
        })
        .unwrap_err();
        assert_eq!(err.message(), "name contains forbidden character '/'");
    }

    #[test]
    fn validate_request_surfaces_range_messages() {
        let err = validate_request(&SizeReq { total_size: 0 }).unwrap_err();
        assert_eq!(err.message(), "total_size must be positive");
    }
}
