use crate::telemetry::types::RejectedTakeMarker;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineSegment {
    pub start_timestamp_us: u64,
    pub end_timestamp_us: u64,
    pub is_rejected: bool,
    pub marker: Option<RejectedTakeMarker>,
}

#[derive(Default)]
pub struct LiveSnipTimelineManager {
    rejected_takes: Vec<RejectedTakeMarker>,
}

impl LiveSnipTimelineManager {
    pub fn new() -> Self {
        Self {
            rejected_takes: Vec::new(),
        }
    }

    pub fn record_rejected_take(&mut self, marker: RejectedTakeMarker) {
        self.rejected_takes.push(marker);
    }

    pub fn rejected_takes(&self) -> &[RejectedTakeMarker] {
        &self.rejected_takes
    }

    /// Check if a given microsecond timestamp falls inside a rejected take
    pub fn is_time_rejected(&self, timestamp_us: u64) -> bool {
        self.rejected_takes.iter().any(|m| {
            timestamp_us >= m.start_timestamp_us && timestamp_us <= m.end_timestamp_us
        })
    }

    /// Partition a timeline interval [0, total_duration_us] into kept vs rejected segments
    pub fn partition_timeline(&self, total_duration_us: u64) -> Vec<TimelineSegment> {
        if self.rejected_takes.is_empty() {
            return vec![TimelineSegment {
                start_timestamp_us: 0,
                end_timestamp_us: total_duration_us,
                is_rejected: false,
                marker: None,
            }];
        }

        // Sort markers by start timestamp
        let mut sorted_markers = self.rejected_takes.clone();
        sorted_markers.sort_by_key(|m| m.start_timestamp_us);

        let mut segments = Vec::new();
        let mut cursor_us = 0u64;

        for marker in sorted_markers {
            let start = marker.start_timestamp_us.min(total_duration_us);
            let end = marker.end_timestamp_us.min(total_duration_us);

            if start > cursor_us {
                segments.push(TimelineSegment {
                    start_timestamp_us: cursor_us,
                    end_timestamp_us: start,
                    is_rejected: false,
                    marker: None,
                });
            }

            if end > start {
                segments.push(TimelineSegment {
                    start_timestamp_us: start,
                    end_timestamp_us: end,
                    is_rejected: true,
                    marker: Some(marker),
                });
            }

            cursor_us = cursor_us.max(end);
        }

        if cursor_us < total_duration_us {
            segments.push(TimelineSegment {
                start_timestamp_us: cursor_us,
                end_timestamp_us: total_duration_us,
                is_rejected: false,
                marker: None,
            });
        }

        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_snip_timeline_partition() {
        let mut manager = LiveSnipTimelineManager::new();

        // 10-second recording: 0 to 10_000_000 us
        let total_us = 10_000_000u64;

        // User pressed Ctrl+Z at t = 8.0s (8_000_000 us), snipping preceding 5.0s (3.0s .. 8.0s)
        let marker = RejectedTakeMarker {
            marker_id: 1,
            start_timestamp_us: 3_000_000,
            end_timestamp_us: 8_000_000,
            duration_seconds: 5.0,
            reason: "Live Snip: Hotkey Ctrl+Z triggered".into(),
        };
        manager.record_rejected_take(marker);

        let segments = manager.partition_timeline(total_us);

        // Expect 3 segments: [0..3s (kept)], [3s..8s (rejected)], [8s..10s (kept)]
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].start_timestamp_us, 0);
        assert_eq!(segments[0].end_timestamp_us, 3_000_000);
        assert!(!segments[0].is_rejected);

        assert_eq!(segments[1].start_timestamp_us, 3_000_000);
        assert_eq!(segments[1].end_timestamp_us, 8_000_000);
        assert!(segments[1].is_rejected);

        assert_eq!(segments[2].start_timestamp_us, 8_000_000);
        assert_eq!(segments[2].end_timestamp_us, 10_000_000);
        assert!(!segments[2].is_rejected);

        // Check point queries
        assert!(!manager.is_time_rejected(2_000_000));
        assert!(manager.is_time_rejected(5_000_000));
        assert!(!manager.is_time_rejected(9_000_000));
    }
}
