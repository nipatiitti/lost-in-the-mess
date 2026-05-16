#[cfg(target_os = "linux")]
pub mod platform {
    use std::time::Duration;

    use app_sdk::{VideoCodec, VideoReceiver, VideoStreamer};
    use tracing::{error, info, warn};

    pub fn stream_video(streamer: VideoStreamer, device_path: String) {
        use v4l::Device;
        use v4l::buffer::Type;
        use v4l::io::mmap::Stream;
        use v4l::io::traits::CaptureStream;
        use v4l::video::Capture;

        std::thread::spawn(move || {
            info!("Starting video capture on {}", device_path);

            let mut dev = match Device::with_path(&device_path) {
                Ok(d) => d,
                Err(e) => {
                    error!("Failed to open video device {}: {}", device_path, e);
                    return;
                }
            };

            let mut format = dev.format().expect("Failed to read format");
            format.width = 640;
            format.height = 480;
            format.fourcc = v4l::FourCC::new(b"MJPG");

            match dev.set_format(&format) {
                Ok(f) => {
                    if f.fourcc != v4l::FourCC::new(b"MJPG") {
                        error!(
                            "Camera does not support MJPG format (got {}). \
                             Please use a camera that supports MJPG.",
                            f.fourcc
                        );
                        return;
                    }
                    info!("Format set to: {}x{} {}", f.width, f.height, f.fourcc);
                }
                Err(e) => {
                    error!("Failed to set format: {}", e);
                    return;
                }
            }

            let mut stream = Stream::with_buffers(&mut dev, Type::VideoCapture, 4)
                .expect("Failed to create capture stream");

            loop {
                let (buf, _meta) = match stream.next() {
                    Ok(res) => res,
                    Err(e) => {
                        warn!("Stream error: {}", e);
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                };

                if let Err(e) = streamer.send_frame(640, 480, buf.to_vec()) {
                    warn!("Failed to send video frame: {}", e);
                }

                std::thread::sleep(Duration::from_millis(100)); // ~10 fps cap
            }
        });
    }

    pub async fn view_video_tcp(
        mut receiver: VideoReceiver,
        mut stream: tokio::net::TcpStream,
    ) {
        use tokio::io::AsyncWriteExt;

        info!("Starting video viewer. Piping frames to TCP client...");
        loop {
            match receiver.recv_frame().await {
                Ok(frame) => {
                    if let Err(e) = stream.write_all(&frame.data).await {
                        error!("TCP write failed, client disconnected: {}", e);
                        break;
                    }
                }
                Err(app_sdk::SdkError::Lagged) => {
                    warn!("Video receiver lagged — some frames dropped");
                }
                Err(_) => break,
            }
        }
    }

    /// Helper: build a VideoStreamer for a Node.
    pub fn make_streamer(node: &app_sdk::Node) -> VideoStreamer {
        VideoStreamer::new(node.clone(), VideoCodec::Mjpeg)
    }

    /// Helper: build a VideoReceiver for a Node.
    pub fn make_receiver(node: &app_sdk::Node) -> VideoReceiver {
        VideoReceiver::new(node)
    }
}

#[cfg(not(target_os = "linux"))]
pub mod platform {
    use app_sdk::{VideoReceiver, VideoStreamer};
    use tracing::error;

    pub fn stream_video(_streamer: VideoStreamer, _device_path: String) {
        error!("Video streaming is only supported on Linux (v4l2)");
    }

    pub async fn view_video_tcp(
        _receiver: VideoReceiver,
        _stream: tokio::net::TcpStream,
    ) {
        error!("Video viewing is only supported on Linux (v4l2)");
    }

    pub fn make_streamer(node: &app_sdk::Node) -> VideoStreamer {
        VideoStreamer::new(node.clone(), app_sdk::VideoCodec::Mjpeg)
    }

    pub fn make_receiver(node: &app_sdk::Node) -> VideoReceiver {
        VideoReceiver::new(node)
    }
}

pub use platform::*;
