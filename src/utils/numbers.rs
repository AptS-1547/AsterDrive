use crate::errors::{AsterError, Result};

pub fn bytes_to_usize(bytes: i64, value_name: &str) -> Result<usize> {
    if bytes < 0 {
        return Err(AsterError::internal_error(format!(
            "{value_name} cannot be negative: {bytes}"
        )));
    }

    usize::try_from(bytes).map_err(|_| {
        AsterError::internal_error(format!(
            "{value_name} exceeds platform usize range: {bytes}"
        ))
    })
}

pub fn i32_to_usize(value: i32, value_name: &str) -> Result<usize> {
    usize::try_from(value).map_err(|_| {
        AsterError::internal_error(format!("{value_name} cannot be negative: {value}"))
    })
}

pub fn i64_to_u64(value: i64, value_name: &str) -> Result<u64> {
    u64::try_from(value).map_err(|_| {
        AsterError::internal_error(format!("{value_name} cannot be negative: {value}"))
    })
}

pub fn usize_to_i32(value: usize, value_name: &str) -> Result<i32> {
    i32::try_from(value)
        .map_err(|_| AsterError::internal_error(format!("{value_name} exceeds i32 range: {value}")))
}

pub fn calc_total_chunks(total_size: i64, chunk_size: i64, context: &str) -> Result<i32> {
    if total_size < 0 {
        return Err(AsterError::validation_error(format!(
            "{context} total_size cannot be negative: {total_size}"
        )));
    }
    if chunk_size <= 0 {
        return Err(AsterError::internal_error(format!(
            "{context} chunk_size must be positive, got {chunk_size}"
        )));
    }

    let adjusted = total_size.checked_add(chunk_size - 1).ok_or_else(|| {
        AsterError::validation_error(format!("{context} total_size is too large: {total_size}"))
    })?;
    let chunks = adjusted / chunk_size;

    i32::try_from(chunks).map_err(|_| {
        AsterError::validation_error(format!("{context} requires too many chunks: {chunks}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_usize_accepts_positive_values() {
        assert_eq!(bytes_to_usize(5_242_880, "chunk_size").unwrap(), 5_242_880);
    }

    #[test]
    fn bytes_to_usize_rejects_negative_values() {
        let err = bytes_to_usize(-1, "chunk_size").unwrap_err();
        assert_eq!(err.code(), "E004");
    }

    #[test]
    fn i32_to_usize_rejects_negative_values() {
        let err = i32_to_usize(-1, "total_chunks").unwrap_err();
        assert_eq!(err.code(), "E004");
    }

    #[test]
    fn i64_to_u64_accepts_positive_values() {
        assert_eq!(i64_to_u64(42, "content_length").unwrap(), 42);
    }

    #[test]
    fn i64_to_u64_rejects_negative_values() {
        let err = i64_to_u64(-1, "content_length").unwrap_err();
        assert_eq!(err.code(), "E004");
    }

    #[test]
    fn usize_to_i32_rejects_overflow() {
        let err = usize_to_i32(i32::MAX as usize + 1, "uploaded_part_count").unwrap_err();
        assert_eq!(err.code(), "E004");
    }

    #[test]
    fn calc_total_chunks_rounds_up() {
        assert_eq!(
            calc_total_chunks(10_485_761, 5_242_880, "multipart upload").unwrap(),
            3
        );
    }

    #[test]
    fn calc_total_chunks_handles_exact_division() {
        assert_eq!(
            calc_total_chunks(10_485_760, 5_242_880, "multipart upload").unwrap(),
            2
        );
    }

    #[test]
    fn calc_total_chunks_allows_zero_size() {
        assert_eq!(calc_total_chunks(0, 5, "multipart upload").unwrap(), 0);
    }

    #[test]
    fn calc_total_chunks_rejects_negative_total_size() {
        let err = calc_total_chunks(-1, 5, "multipart upload").unwrap_err();
        assert_eq!(err.code(), "E005");
    }

    #[test]
    fn calc_total_chunks_rejects_non_positive_chunk_size() {
        let err = calc_total_chunks(10, 0, "multipart upload").unwrap_err();
        assert_eq!(err.code(), "E004");
    }

    #[test]
    fn calc_total_chunks_rejects_i32_overflow() {
        let overflow_total_size = (i64::from(i32::MAX) + 1) * 5;
        let err = calc_total_chunks(overflow_total_size, 1, "multipart upload").unwrap_err();
        assert_eq!(err.code(), "E005");
    }
}
