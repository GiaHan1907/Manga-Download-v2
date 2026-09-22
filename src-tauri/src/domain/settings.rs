use serde::{Deserialize, Serialize};

use super::SettingsPatchInput;
use crate::error::AppError;

pub const CHAPTER_CONCURRENCY_MIN: i64 = 1;
pub const CHAPTER_CONCURRENCY_MAX: i64 = 5;
pub const IMAGE_CONCURRENCY_MIN: i64 = 1;
pub const IMAGE_CONCURRENCY_MAX: i64 = 8;

const READER_MODES: [&str; 3] = ["single", "double", "vertical"];
const READING_DIRECTIONS: [&str; 2] = ["ltr", "rtl"];
const OUTPUT_FORMATS: [&str; 3] = ["cbz", "folder", "pdf"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub default_reader_mode: String,
    pub default_reading_direction: String,
    pub chapter_concurrency: i64,
    pub image_concurrency: i64,
    pub output_format: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            default_reader_mode: "single".to_string(),
            default_reading_direction: "rtl".to_string(),
            chapter_concurrency: 2,
            image_concurrency: 4,
            output_format: "cbz".to_string(),
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), AppError> {
        if !READER_MODES.contains(&self.default_reader_mode.as_str()) {
            return Err(AppError::validation("defaultReaderMode không hợp lệ."));
        }
        if !READING_DIRECTIONS.contains(&self.default_reading_direction.as_str()) {
            return Err(AppError::validation(
                "defaultReadingDirection không hợp lệ.",
            ));
        }
        if !(CHAPTER_CONCURRENCY_MIN..=CHAPTER_CONCURRENCY_MAX).contains(&self.chapter_concurrency)
        {
            return Err(AppError::validation(
                "chapterConcurrency phải là số nguyên từ 1 đến 5.",
            ));
        }
        if !(IMAGE_CONCURRENCY_MIN..=IMAGE_CONCURRENCY_MAX).contains(&self.image_concurrency) {
            return Err(AppError::validation(
                "imageConcurrency phải là số nguyên từ 1 đến 8.",
            ));
        }
        if !OUTPUT_FORMATS.contains(&self.output_format.as_str()) {
            return Err(AppError::validation("outputFormat không hợp lệ."));
        }
        Ok(())
    }

    pub fn merged_with_patch(&self, patch: SettingsPatchInput) -> Self {
        let mut next = self.clone();
        if let Some(value) = patch.default_reader_mode {
            next.default_reader_mode = value;
        }
        if let Some(value) = patch.default_reading_direction {
            next.default_reading_direction = value;
        }
        if let Some(value) = patch.chapter_concurrency {
            next.chapter_concurrency = value;
        }
        if let Some(value) = patch.image_concurrency {
            next.image_concurrency = value;
        }
        if let Some(value) = patch.output_format {
            next.output_format = value;
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert!(AppSettings::default().validate().is_ok());
    }

    #[test]
    fn bounds_are_enforced() {
        let bad = AppSettings {
            chapter_concurrency: 0,
            ..AppSettings::default()
        };
        assert!(bad.validate().is_err());
        let bad = AppSettings {
            chapter_concurrency: 6,
            ..AppSettings::default()
        };
        assert!(bad.validate().is_err());
        let bad = AppSettings {
            image_concurrency: 9,
            ..AppSettings::default()
        };
        assert!(bad.validate().is_err());
        let bad = AppSettings {
            default_reader_mode: "carousel".to_string(),
            ..AppSettings::default()
        };
        assert!(bad.validate().is_err());
        let bad = AppSettings {
            output_format: "epub".to_string(),
            ..AppSettings::default()
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn patch_rejects_unknown_keys_and_maps_camel_case() {
        let bad = serde_json::from_str::<super::SettingsPatchInput>(
            "{\"chapterConcurrency\":2,\"evilKey\":\"DROP TABLE\"}",
        );
        assert!(
            bad.is_err(),
            "unknown settings keys must be rejected outright"
        );

        let good = serde_json::from_str::<super::SettingsPatchInput>(
            "{\"defaultReaderMode\":\"double\",\"chapterConcurrency\":3}",
        )
        .unwrap();
        assert_eq!(good.chapter_concurrency, Some(3));
        assert_eq!(good.output_format, None);
    }

    #[test]
    fn serialized_keys_are_camel_case() {
        let value = serde_json::to_value(AppSettings::default()).unwrap();
        // serde_json maps are key-sorted; assert the exact camelCase set.
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        let mut expected = vec![
            "defaultReaderMode",
            "defaultReadingDirection",
            "chapterConcurrency",
            "imageConcurrency",
            "outputFormat",
        ];
        expected.sort();
        assert_eq!(keys, expected);
    }
}
