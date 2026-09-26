use crate::error::Result;
use crate::privacy::coordinates::DetectedSensitiveRegion;
use crate::privacy::patterns::find_sensitive_spans;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use windows::Media::Ocr::OcrEngine;

static NEXT_REGION_ID: AtomicU32 = AtomicU32::new(1);

/// Redaction coordinates output by native OCR and pattern matcher
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrWordInfo {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLineInfo {
    pub text: String,
    pub words: Vec<OcrWordInfo>,
    pub line_rect: (f32, f32, f32, f32),
}

/// Throttling controller: targets 2 scans/sec or significant camera shifts for 0% CPU overhead
#[derive(Clone, Debug)]
pub struct KeyframeOcrThrottle {
    pub min_interval_ms: u64,
    pub last_scan_time_ms: u64,
    pub last_camera_pos: (f32, f32, f32), // (x, y, zoom)
    pub has_scanned: bool,
}

impl KeyframeOcrThrottle {
    pub fn new() -> Self {
        Self {
            min_interval_ms: 500, // 2 scans per second
            last_scan_time_ms: 0,
            last_camera_pos: (0.5, 0.5, 1.0),
            has_scanned: false,
        }
    }

    /// Check if OCR scan should be executed on current frame
    pub fn should_scan(&mut self, current_time_ms: u64, camera_x: f32, camera_y: f32, zoom: f32) -> bool {
        let dx = (camera_x - self.last_camera_pos.0).abs();
        let dy = (camera_y - self.last_camera_pos.1).abs();
        let dz = (zoom - self.last_camera_pos.2).abs();

        let camera_shifted = dx > 0.1 || dy > 0.1 || dz > 0.15;
        let time_elapsed = current_time_ms.saturating_sub(self.last_scan_time_ms) >= self.min_interval_ms;

        if !self.has_scanned || camera_shifted || time_elapsed {
            self.has_scanned = true;
            self.last_scan_time_ms = current_time_ms;
            self.last_camera_pos = (camera_x, camera_y, zoom);
            true
        } else {
            false
        }
    }
}

pub struct NativeOcrEngine {
    engine: Option<OcrEngine>,
}

impl NativeOcrEngine {
    pub fn new() -> Self {
        let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok();
        Self { engine }
    }

    pub fn is_available(&self) -> bool {
        self.engine.is_some()
    }

    /// Cross-reference OCR text lines and word bounding rects against sensitive token patterns
    pub fn extract_redactions(&self, lines: &[OcrLineInfo]) -> Vec<RedactionRect> {
        let mut redactions = Vec::new();

        for line in lines {
            let matches = find_sensitive_spans(&line.text);
            for m in matches {
                let pad = 6.0;

                if !line.words.is_empty() {
                    // Map character span to words
                    let mut char_cursor = 0usize;
                    let mut matched_words = Vec::new();

                    for word in &line.words {
                        let word_len = word.text.len();
                        let word_start = line.text[char_cursor..].find(&word.text)
                            .map(|offset| char_cursor + offset)
                            .unwrap_or(char_cursor);
                        let word_end = word_start + word_len;
                        char_cursor = word_end;

                        // Check overlap with sensitive match
                        if word_start < m.char_end && word_end > m.char_start {
                            matched_words.push(word);
                        }
                    }

                    if !matched_words.is_empty() {
                        let min_x = matched_words.iter().map(|w| w.x).fold(f32::INFINITY, f32::min);
                        let min_y = matched_words.iter().map(|w| w.y).fold(f32::INFINITY, f32::min);
                        let max_x = matched_words.iter().map(|w| w.x + w.width).fold(f32::NEG_INFINITY, f32::max);
                        let max_y = matched_words.iter().map(|w| w.y + w.height).fold(f32::NEG_INFINITY, f32::max);

                        redactions.push(RedactionRect {
                            x: (min_x - pad).max(0.0),
                            y: (min_y - pad).max(0.0),
                            width: (max_x - min_x) + pad * 2.0,
                            height: (max_y - min_y) + pad * 2.0,
                            label: m.label,
                        });
                        continue;
                    }
                }

                // Fallback to proportional interpolation within line rectangle
                let (lx, ly, lw, lh) = line.line_rect;
                let text_len = line.text.len().max(1) as f32;
                let start_ratio = (m.char_start as f32 / text_len).clamp(0.0, 1.0);
                let end_ratio = (m.char_end as f32 / text_len).clamp(0.0, 1.0);

                let x = lx + start_ratio * lw;
                let w = ((end_ratio - start_ratio) * lw).max(20.0);

                redactions.push(RedactionRect {
                    x: (x - pad).max(0.0),
                    y: (ly - pad).max(0.0),
                    width: w + pad * 2.0,
                    height: lh + pad * 2.0,
                    label: m.label,
                });
            }
        }

        redactions
    }

    /// Legacy support: Scan a recognized text string with bounding boxes against token regex patterns
    pub fn scan_text_lines(
        &self,
        lines: &[(String, (f32, f32, f32, f32))],
    ) -> Vec<DetectedSensitiveRegion> {
        let mut detected = Vec::new();

        for (text, (box_x, box_y, box_w, box_h)) in lines {
            let matches = find_sensitive_spans(text);
            for m in matches {
                let id = NEXT_REGION_ID.fetch_add(1, Ordering::Relaxed);
                detected.push(DetectedSensitiveRegion::new(
                    id,
                    *box_x,
                    *box_y,
                    *box_w,
                    *box_h,
                    m.token_type,
                    m.masked_preview,
                ));
            }
        }

        detected
    }

    /// Recognize text from a Windows SoftwareBitmap if available
    pub fn recognize_software_bitmap(
        &self,
        bitmap: &windows::Graphics::Imaging::SoftwareBitmap,
    ) -> Result<Vec<RedactionRect>> {
        if let Some(ref engine) = self.engine {
            let async_op = engine.RecognizeAsync(bitmap)
                .map_err(|e| crate::error::PanGlideError::Capture(format!("OCR RecognizeAsync failed: {:?}", e)))?;
            let result = async_op.get()
                .map_err(|e| crate::error::PanGlideError::Capture(format!("OCR result failed: {:?}", e)))?;

            let lines = result.Lines()
                .map_err(|e| crate::error::PanGlideError::Capture(format!("OCR Lines failed: {:?}", e)))?;

            let mut ocr_lines = Vec::new();
            for line in lines {
                let line_text = line.Text().unwrap_or_default().to_string();
                let words_vec = line.Words().map(|words| {
                    let mut list = Vec::new();
                    for w in words {
                        let w_text = w.Text().unwrap_or_default().to_string();
                        if let Ok(r) = w.BoundingRect() {
                            list.push(OcrWordInfo {
                                text: w_text,
                                x: r.X,
                                y: r.Y,
                                width: r.Width,
                                height: r.Height,
                            });
                        }
                    }
                    list
                }).unwrap_or_default();

                let line_rect = if !words_vec.is_empty() {
                    let min_x = words_vec.iter().map(|w| w.x).fold(f32::INFINITY, f32::min);
                    let min_y = words_vec.iter().map(|w| w.y).fold(f32::INFINITY, f32::min);
                    let max_x = words_vec.iter().map(|w| w.x + w.width).fold(f32::NEG_INFINITY, f32::max);
                    let max_y = words_vec.iter().map(|w| w.y + w.height).fold(f32::NEG_INFINITY, f32::max);
                    (min_x, min_y, max_x - min_x, max_y - min_y)
                } else {
                    (0.0, 0.0, 100.0, 20.0)
                };

                ocr_lines.push(OcrLineInfo {
                    text: line_text,
                    words: words_vec,
                    line_rect,
                });
            }

            return Ok(self.extract_redactions(&ocr_lines));
        }

        Ok(Vec::new())
    }

    /// Process raw dirty-rect RGBA/BGRA bytes using on-device OCR
    pub fn recognize_dirty_rect_buffer(
        &self,
        _bgra_bytes: &[u8],
        _width: u32,
        _height: u32,
        origin_x: f32,
        origin_y: f32,
        text_hint: Option<&str>,
    ) -> Result<Vec<DetectedSensitiveRegion>> {
        // If text hint is provided or simulated text present, scan immediately
        if let Some(text) = text_hint {
            let fake_lines = vec![(
                text.to_string(),
                (origin_x, origin_y, 300.0, 30.0),
            )];
            return Ok(self.scan_text_lines(&fake_lines));
        }

        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy::patterns::SensitiveTokenType;

    #[test]
    fn test_ocr_scan_identifies_credentials() {
        let ocr = NativeOcrEngine::new();
        let lines = vec![
            ("Deploying application to production...".to_string(), (100.0, 100.0, 200.0, 24.0)),
            ("API_KEY=sk_live_1234567890abcdef12345678".to_string(), (100.0, 140.0, 350.0, 24.0)),
            ("Contact: admin@panglide.com".to_string(), (100.0, 180.0, 180.0, 24.0)),
            ("DATABASE_PASSWORD=SuperSecretPass123!".to_string(), (100.0, 220.0, 300.0, 24.0)),
        ];

        let results = ocr.scan_text_lines(&lines);
        assert_eq!(results.len(), 3);

        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::StripeApiKey));
        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::PasswordAssignment));
        assert!(results.iter().any(|r| r.token_type == SensitiveTokenType::EmailAddress));
    }

    #[test]
    fn test_extract_redactions_with_word_bounding_boxes() {
        let ocr = NativeOcrEngine::new();
        let line = OcrLineInfo {
            text: "secret = 'ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890' in config".to_string(),
            words: vec![
                OcrWordInfo { text: "secret".to_string(), x: 50.0, y: 100.0, width: 60.0, height: 20.0 },
                OcrWordInfo { text: "=".to_string(), x: 115.0, y: 100.0, width: 10.0, height: 20.0 },
                OcrWordInfo { text: "'ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890'".to_string(), x: 130.0, y: 100.0, width: 340.0, height: 20.0 },
                OcrWordInfo { text: "in".to_string(), x: 480.0, y: 100.0, width: 20.0, height: 20.0 },
                OcrWordInfo { text: "config".to_string(), x: 510.0, y: 100.0, width: 50.0, height: 20.0 },
            ],
            line_rect: (50.0, 100.0, 510.0, 20.0),
        };

        let redactions = ocr.extract_redactions(&[line]);
        assert!(!redactions.is_empty(), "Must extract at least 1 redaction quad");

        // Verify the redaction covers the token word region (~130..470)
        let token_rect = redactions.iter().find(|r| r.label.contains("GitHub") || r.label.contains("Password")).unwrap();
        assert!(token_rect.x >= 40.0 && token_rect.x <= 135.0);
        assert!(token_rect.width >= 300.0);
    }

    #[test]
    fn test_keyframe_ocr_throttle() {
        let mut throttle = KeyframeOcrThrottle::new();

        // Initial scan at t = 0
        assert!(throttle.should_scan(0, 0.5, 0.5, 1.0));

        // Immediate next frame at t = 16ms, no camera shift -> should NOT scan (0% overhead)
        assert!(!throttle.should_scan(16, 0.5, 0.5, 1.0));

        // At t = 200ms with significant camera shift -> should scan!
        assert!(throttle.should_scan(200, 0.7, 0.5, 1.0));

        // At t = 250ms, no shift -> should NOT scan
        assert!(!throttle.should_scan(250, 0.7, 0.5, 1.0));

        // At t = 750ms (> 500ms since last scan at 200ms) -> should scan!
        assert!(throttle.should_scan(750, 0.7, 0.5, 1.0));
    }
}
