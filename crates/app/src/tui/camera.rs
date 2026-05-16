use std::sync::Arc;
use std::time::{Duration, Instant};

use app_sdk::{VideoChannel, VideoQuality, VideoStreamer};
use image::ImageEncoder;
use image::codecs::jpeg::JpegEncoder;
use nokhwa::{
    Camera,
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
};
use tokio::sync::mpsc;

/// Target capture resolution — we ask nokhwa for this; actual camera may differ.
const CAPTURE_WIDTH: u32 = 640;
const CAPTURE_HEIGHT: u32 = 480;

/// Max outgoing frame rate. Excess frames are dropped so the radio isn't flooded.
const MAX_FPS: u64 = 15;
const FRAME_BUDGET: Duration = Duration::from_millis(1000 / MAX_FPS);

pub fn run_capture(
    video_ch: Arc<VideoChannel>,
    quality: VideoQuality,
    mut cmd_rx: mpsc::Receiver<bool>,
    preview_tx: mpsc::Sender<Vec<u8>>,
) {
    loop {
        match cmd_rx.blocking_recv() {
            Some(true) => {}
            Some(false) => continue,
            None => return,
        }

        let mut camera = match Camera::new(
            CameraIndex::Index(0),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::HighestResolution(
                nokhwa::utils::Resolution::new(CAPTURE_WIDTH, CAPTURE_HEIGHT),
            )),
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("camera open failed: {e}");
                continue;
            }
        };

        if camera.open_stream().is_err() {
            continue;
        }

        let streamer = VideoStreamer::new(Arc::clone(&video_ch), quality);
        let res = camera.resolution();
        let cap_w = res.width();
        let cap_h = res.height();
        let mut last_sent = Instant::now() - FRAME_BUDGET;

        loop {
            match cmd_rx.try_recv() {
                Ok(false) | Err(mpsc::error::TryRecvError::Disconnected) => break,
                _ => {}
            }

            let frame = match camera.frame() {
                Ok(f) => f,
                Err(_) => continue,
            };

            let decoded = match frame.decode_image::<RgbFormat>() {
                Ok(img) => img,
                Err(_) => continue,
            };
            let rgb = decoded.as_raw();

            // Local preview — always send for UI feedback regardless of rate limit
            let mut jpeg_buf = Vec::new();
            let enc = JpegEncoder::new_with_quality(&mut jpeg_buf, 50);
            if enc.write_image(rgb, cap_w, cap_h, image::ExtendedColorType::Rgb8).is_ok() {
                let _ = preview_tx.try_send(jpeg_buf);
            }

            // Rate-limit the radio stream
            let now = Instant::now();
            if now.duration_since(last_sent) < FRAME_BUDGET {
                continue;
            }
            last_sent = now;

            let _ = streamer.send_frame(rgb, cap_w, cap_h);
        }

        let _ = camera.stop_stream();
    }
}
