#[cfg(target_os = "linux")]
pub mod platform {
    use std::sync::Arc;
    use std::time::Duration;
    use v4l::buffer::Type;
    use v4l::io::traits::CaptureStream;
    use v4l::io::mmap::Stream;
    use v4l::video::Capture;
    use v4l::Device;
    use litm_common::{Delivery, ObjectId, SendPolicy};
    use tracing::{info, warn, error};
    use tokio::io::AsyncWriteExt;

    pub fn stream_video(delivery: Arc<dyn Delivery>, device_path: String) {
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
                        error!("Camera does not support MJPG format (got {}). Please use a camera that supports MJPG.", f.fourcc);
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
                
            let mut object_id: ObjectId = rand::random();
            
            loop {
                // Read frame
                let (buf, _meta) = match stream.next() {
                    Ok(res) => res,
                    Err(e) => {
                        warn!("Stream error: {}", e);
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                };
                
                let payload = buf.to_vec();
                
                let policy = SendPolicy {
                    ttl: Duration::from_millis(1500),
                    desired_coverage: 1, // Hackathon default
                    priority: 64, // Lower priority than telemetry
                };
                
                if let Err(e) = delivery.send_object(object_id, payload, policy) {
                    warn!("Failed to send video frame: {}", e);
                }
                
                object_id = object_id.wrapping_add(1);
                
                // Simple framerate limit to avoid congestion
                std::thread::sleep(Duration::from_millis(100)); // Max ~10 fps
            }
        });
    }

    pub async fn view_video_tcp(delivery: Arc<dyn Delivery>, mut stream: tokio::net::TcpStream) {
        info!("Starting video viewer. Piping frames to TCP client...");
        let mut rx = delivery.subscribe();
        let mut expected_id: Option<ObjectId> = None;
        
        while let Some(obj) = rx.recv().await {
            let is_newer = match expected_id {
                None => true,
                Some(expected) => {
                    let diff = obj.id.wrapping_sub(expected);
                    diff < 10000 || diff > (u32::MAX - 10000)
                }
            };
            
            if is_newer {
                if let Err(e) = stream.write_all(&obj.payload).await {
                    error!("TCP write failed, client disconnected: {}", e);
                    break;
                }
                expected_id = Some(obj.id.wrapping_add(1));
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub mod platform {
    use std::sync::Arc;
    use litm_common::Delivery;
    use tracing::error;

    pub fn stream_video(_delivery: Arc<dyn Delivery>, _device_path: String) {
        error!("Video streaming is only supported on Linux (v4l2)");
    }

    pub async fn view_video_tcp(_delivery: Arc<dyn Delivery>, _stream: tokio::net::TcpStream) {
        error!("Video viewing is only supported on Linux (v4l2)");
    }
}

pub use platform::*;
