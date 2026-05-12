//! File source abstraction for uploads.

use std::path::{Path, PathBuf};

/// A source for file data that will be uploaded.
///
/// Construct with [`FileSource::bytes`] or [`FileSource::path`], or convert
/// from [`PathBuf`]/[`&Path`] via the `From` impls.
#[derive(Debug)]
pub enum FileSource {
    /// Raw bytes with explicit filename and content type.
    Bytes {
        /// File name to send.
        filename: String,
        /// Raw file data.
        bytes: Vec<u8>,
        /// MIME content type.
        content_type: String,
    },
    /// A filesystem path. Resolved at upload time.
    Path(PathBuf),
}

impl FileSource {
    /// Create a `Bytes` variant from explicit parts.
    pub fn bytes(
        filename: impl Into<String>,
        data: impl Into<Vec<u8>>,
        content_type: impl Into<String>,
    ) -> Self {
        Self::Bytes {
            filename: filename.into(),
            bytes: data.into(),
            content_type: content_type.into(),
        }
    }

    /// Create a `Path` variant.
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }
}

impl From<PathBuf> for FileSource {
    fn from(p: PathBuf) -> Self {
        Self::Path(p)
    }
}

impl From<&Path> for FileSource {
    fn from(p: &Path) -> Self {
        Self::Path(p.to_path_buf())
    }
}

/// Resolve a [`FileSource`] into `(filename, bytes, content_type)`.
///
/// For the `Bytes` variant the fields are returned directly.
/// For the `Path` variant the file is read, the filename is extracted from
/// the final path component, and the MIME type is guessed from the extension
/// (falling back to `application/octet-stream`).
#[expect(dead_code)]
pub(crate) async fn resolve_to_bytes(
    src: FileSource,
) -> std::io::Result<(String, Vec<u8>, String)> {
    match src {
        FileSource::Bytes {
            filename,
            bytes,
            content_type,
        } => Ok((filename, bytes, content_type)),
        FileSource::Path(p) => {
            let data = tokio::fs::read(&p).await?;
            let filename = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let content_type = mime_guess::from_path(&p)
                .first_or_octet_stream()
                .to_string();
            Ok((filename, data, content_type))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn file_source_path_resolves_filename_and_content_type() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("report.pdf");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            f.write_all(b"%PDF-1.4 fake").unwrap();
        }

        let src = FileSource::path(&file_path);
        let (name, data, ctype) = resolve_to_bytes(src).await.unwrap();

        assert_eq!(name, "report.pdf");
        assert_eq!(data, b"%PDF-1.4 fake".as_slice());
        assert_eq!(ctype, "application/pdf");
    }

    #[tokio::test]
    async fn file_source_unknown_extension_falls_back_to_octet_stream() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.unknownext");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            f.write_all(b"hello").unwrap();
        }

        let src = FileSource::path(&file_path);
        let (_, _, ctype) = resolve_to_bytes(src).await.unwrap();

        assert_eq!(ctype, "application/octet-stream");
    }

    #[tokio::test]
    async fn file_source_path_nonexistent_returns_io_error() {
        let src = FileSource::path("/tmp/honcho_test_nonexistent_42deadbeef.pdf");
        let result = resolve_to_bytes(src).await;
        assert!(result.is_err());
    }
}
