use std::fs;

use img_parts::{ImageEXIF, ImageICC};

use crate::modules::compress_photos::models::{PhotoAsset, PhotoFormat};

#[derive(Clone, Default)]
pub(super) struct SourceMetadata {
    pub(super) exif: Option<img_parts::Bytes>,
    pub(super) icc_profile: Option<img_parts::Bytes>,
}

pub(super) fn apply_container_metadata(
    encoded: Vec<u8>,
    output_format: PhotoFormat,
    metadata: &SourceMetadata,
) -> Result<Vec<u8>, String> {
    match output_format {
        PhotoFormat::Jpeg => {
            let mut image = img_parts::jpeg::Jpeg::from_bytes(encoded.into())
                .map_err(|error| format!("Could not restore JPEG metadata: {error}"))?;
            image.set_exif(metadata.exif.clone());
            image.set_icc_profile(metadata.icc_profile.clone());
            let mut output = Vec::new();
            image
                .encoder()
                .write_to(&mut output)
                .map_err(|error| format!("Could not write JPEG metadata: {error}"))?;
            Ok(output)
        }
        PhotoFormat::Png => {
            let mut image = img_parts::png::Png::from_bytes(encoded.into())
                .map_err(|error| format!("Could not restore PNG metadata: {error}"))?;
            image.set_exif(metadata.exif.clone());
            image.set_icc_profile(metadata.icc_profile.clone());
            let mut output = Vec::new();
            image
                .encoder()
                .write_to(&mut output)
                .map_err(|error| format!("Could not write PNG metadata: {error}"))?;
            Ok(output)
        }
        PhotoFormat::WebP => {
            let mut image = img_parts::webp::WebP::from_bytes(encoded.into())
                .map_err(|error| format!("Could not restore WebP metadata: {error}"))?;
            image.set_exif(metadata.exif.clone());
            image.set_icc_profile(metadata.icc_profile.clone());
            let mut output = Vec::new();
            image
                .encoder()
                .write_to(&mut output)
                .map_err(|error| format!("Could not write WebP metadata: {error}"))?;
            Ok(output)
        }
        PhotoFormat::Avif => Ok(encoded),
    }
}

pub(super) fn read_source_metadata(asset: &PhotoAsset) -> SourceMetadata {
    let bytes = match fs::read(&asset.path) {
        Ok(bytes) => bytes,
        Err(_) => return SourceMetadata::default(),
    };

    match asset.format {
        PhotoFormat::Jpeg => img_parts::jpeg::Jpeg::from_bytes(bytes.into())
            .map(|image| SourceMetadata {
                exif: image.exif(),
                icc_profile: image.icc_profile(),
            })
            .unwrap_or_default(),
        PhotoFormat::Png => img_parts::png::Png::from_bytes(bytes.into())
            .map(|image| SourceMetadata {
                exif: image.exif(),
                icc_profile: image.icc_profile(),
            })
            .unwrap_or_default(),
        PhotoFormat::WebP => img_parts::webp::WebP::from_bytes(bytes.into())
            .map(|image| SourceMetadata {
                exif: image.exif(),
                icc_profile: image.icc_profile(),
            })
            .unwrap_or_default(),
        PhotoFormat::Avif => SourceMetadata::default(),
    }
}
