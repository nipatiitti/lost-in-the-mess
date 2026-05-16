use litm_common::SendPolicy;
use std::time::Duration;

/// Stop once 1 peer confirms. Long TTL, high priority.
pub fn reliable() -> SendPolicy {
    SendPolicy { desired_coverage: 1, ttl: Duration::from_secs(30), priority: 200 }
}

/// Fire and forget: short TTL, low priority. Still requires 1 delivery to stop.
/// Note: desired_coverage=0 causes RaptorQDelivery to send zero packets; use 1.
pub fn best_effort() -> SendPolicy {
    SendPolicy { desired_coverage: 1, ttl: Duration::from_secs(5), priority: 64 }
}

/// Flood to all visible peers. Maximum coverage and priority.
pub fn broadcast() -> SendPolicy {
    SendPolicy { desired_coverage: 255, ttl: Duration::from_secs(60), priority: 255 }
}

/// Brief TTL for video frames; one missed frame is acceptable.
pub fn video_frame() -> SendPolicy {
    SendPolicy { desired_coverage: 1, ttl: Duration::from_millis(1500), priority: 64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering() {
        assert!(reliable().priority > best_effort().priority);
        assert!(broadcast().priority >= reliable().priority);
    }

    #[test]
    fn broadcast_max_coverage() {
        assert_eq!(broadcast().desired_coverage, 255);
    }

    #[test]
    fn video_frame_shortest_ttl() {
        assert!(video_frame().ttl < best_effort().ttl);
        assert!(video_frame().ttl < reliable().ttl);
    }

    #[test]
    fn no_zero_coverage() {
        // desired_coverage=0 is a footgun in RaptorQDelivery — none of our presets use it
        assert!(reliable().desired_coverage > 0);
        assert!(best_effort().desired_coverage > 0);
        assert!(broadcast().desired_coverage > 0);
        assert!(video_frame().desired_coverage > 0);
    }
}
