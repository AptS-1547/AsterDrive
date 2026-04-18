//! 用户资料服务子模块：`avatar_image`。

use std::io::Cursor;

use actix_multipart::Multipart;
use futures::StreamExt;
use image::ImageFormat;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageReader, Limits};

use crate::errors::{AsterError, MapAsterErr, Result};

use super::shared::{AVATAR_SIZE_LG, AVATAR_SIZE_SM, MAX_AVATAR_DECODE_ALLOC};

fn encode_webp(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::WebP)
        .map_aster_err_ctx("encode webp", AsterError::file_upload_failed)?;
    Ok(buf.into_inner())
}

pub(super) fn process_avatar_image(data: Vec<u8>) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_aster_err_ctx("guess avatar format", AsterError::file_type_not_allowed)?;

    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_AVATAR_DECODE_ALLOC);
    reader.limits(limits);

    let img = reader
        .decode()
        .map_aster_err_ctx("decode avatar", AsterError::file_type_not_allowed)?;

    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Err(AsterError::validation_error("empty image"));
    }

    let side = width.min(height);
    let left = (width - side) / 2;
    let top = (height - side) / 2;
    let square = img.crop_imm(left, top, side, side);

    let large = square.resize_exact(AVATAR_SIZE_LG, AVATAR_SIZE_LG, FilterType::Triangle);
    let small = square.resize_exact(AVATAR_SIZE_SM, AVATAR_SIZE_SM, FilterType::Triangle);

    let large_bytes = encode_webp(&large)?;
    let small_bytes = encode_webp(&small)?;
    Ok((small_bytes, large_bytes))
}

pub(super) async fn read_avatar_upload(
    payload: &mut Multipart,
    max_upload_size: usize,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut saw_file = false;

    while let Some(field) = payload.next().await {
        let mut field = field.map_aster_err(AsterError::file_upload_failed)?;
        let has_filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename())
            .is_some();
        if !has_filename {
            while let Some(chunk) = field.next().await {
                chunk.map_aster_err(AsterError::file_upload_failed)?;
            }
            continue;
        }

        saw_file = true;
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_aster_err(AsterError::file_upload_failed)?;
            if bytes.len() + chunk.len() > max_upload_size {
                return Err(AsterError::file_too_large(format!(
                    "avatar upload exceeds {} bytes",
                    max_upload_size
                )));
            }
            bytes.extend_from_slice(&chunk);
        }
        break;
    }

    if !saw_file || bytes.is_empty() {
        return Err(AsterError::validation_error("avatar file is required"));
    }

    Ok(bytes)
}
