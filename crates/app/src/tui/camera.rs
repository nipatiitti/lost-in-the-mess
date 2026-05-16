use app_sdk::{VideoCodec, VideoStreamer};
use nokhwa::{
    Camera,
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
};
use tokio::sync::mpsc;

pub fn run_capture(
    node: app_sdk::Node,
    mut cmd_rx: mpsc::Receiver<bool>,
    preview_tx: mpsc::Sender<image::DynamicImage>,
) {
    loop {
        // Wait for a start command
        match cmd_rx.blocking_recv() {
            Some(true) => {}
            Some(false) => continue,
            None => return,
        }

        let mut camera = match Camera::new(
            CameraIndex::Index(0),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate),
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

        let streamer = VideoStreamer::new(node.clone(), VideoCodec::Mjpeg);
        let res = camera.resolution();
        let width = res.width() as u16;
        let height = res.height() as u16;

        loop {
            match cmd_rx.try_recv() {
                Ok(false) | Err(mpsc::error::TryRecvError::Disconnected) => break,
                _ => {}
            }

            let frame = match camera.frame() {
                Ok(f) => f,
                Err(_) => continue,
            };

            let raw = frame.buffer().to_vec();

            let _ = streamer.send_frame(width, height, raw.clone());

            if let Ok(img) = image::load_from_memory_with_format(&raw, image::ImageFormat::Jpeg) {
                let _ = preview_tx.try_send(img);
            }
        }

        let _ = camera.stop_stream();
    }
}
