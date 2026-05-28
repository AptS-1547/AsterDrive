use crate::entities::file;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveFormat {
    Zip,
    SevenZip,
}

impl ArchiveFormat {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZip => "7z",
        }
    }

    pub(crate) const fn raw_manifest_cache_name(self) -> &'static str {
        match self {
            Self::Zip => "zip_raw_manifest.v1",
            Self::SevenZip => "7z_raw_manifest.v1",
        }
    }

    pub(crate) const fn temp_file_name(self) -> &'static str {
        match self {
            Self::Zip => "source.zip",
            Self::SevenZip => "source.7z",
        }
    }

    pub(crate) fn strip_extension<'a>(self, name: &'a str) -> Option<&'a str> {
        let extension = match self {
            Self::Zip => ".zip",
            Self::SevenZip => ".7z",
        };
        if ends_with_ignore_ascii_case(name, extension) && name.len() > extension.len() {
            Some(&name[..name.len() - extension.len()])
        } else {
            None
        }
    }
}

pub(crate) fn detect_archive_extract_format(source_file: &file::Model) -> Option<ArchiveFormat> {
    if ends_with_ignore_ascii_case(&source_file.name, ".zip") {
        return Some(ArchiveFormat::Zip);
    }
    if ends_with_ignore_ascii_case(&source_file.name, ".7z") {
        return Some(ArchiveFormat::SevenZip);
    }
    None
}

pub(crate) fn detect_archive_preview_format(source_file: &file::Model) -> Option<ArchiveFormat> {
    let mime = source_file.mime_type.to_ascii_lowercase();
    if ends_with_ignore_ascii_case(&source_file.name, ".zip")
        || matches!(
            mime.as_str(),
            "application/zip" | "application/x-zip-compressed"
        )
    {
        return Some(ArchiveFormat::Zip);
    }
    if ends_with_ignore_ascii_case(&source_file.name, ".7z")
        || matches!(
            mime.as_str(),
            "application/x-7z" | "application/x-7z-compressed"
        )
    {
        return Some(ArchiveFormat::SevenZip);
    }
    None
}

fn ends_with_ignore_ascii_case(value: &str, suffix: &str) -> bool {
    value
        .get(value.len().saturating_sub(suffix.len())..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
}
