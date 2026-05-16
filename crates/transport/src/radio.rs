//! kova-wfb-rs adapter: TX/RX OS threads + iw-based channel switching.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver as XReceiver, Sender as XSender, bounded};
use tracing::{debug, error, warn};
use wfb_rs::{WFB_FRAME_TYPE_DATA, WfbRx, WfbRxConfig, WfbTx, WfbTxConfig};

use litm_common::{Error, Result};

#[derive(Clone, Debug)]
pub struct RadioConfig {
    pub iface: String,
    pub stream_id: u32,
    pub mcs_index: u8,
    pub bandwidth: u8,
    pub ring_size: usize,
}

impl Default for RadioConfig {
    fn default() -> Self {
        Self {
            iface: "wlan0".into(),
            stream_id: 0xA11CE,
            mcs_index: 1,
            bandwidth: 40,
            ring_size: 16,
        }
    }
}

pub struct RxFrame {
    pub bytes: Vec<u8>,
    pub rssi_dbm: i8,
}

pub struct Radio {
    pub tx_queue: XSender<Vec<u8>>,
    pub rx_queue: XReceiver<RxFrame>,
    cfg: RadioConfig,
    shutdown: Arc<AtomicBool>,
}

impl Radio {
    pub fn start(cfg: RadioConfig) -> Result<Self> {
        let tx_cfg = WfbTxConfig {
            iface: cfg.iface.clone(),
            stream_id: cfg.stream_id,
            frame_type: WFB_FRAME_TYPE_DATA,
            mcs_index: cfg.mcs_index,
            bandwidth: cfg.bandwidth,
        };
        let rx_cfg = WfbRxConfig {
            iface: cfg.iface.clone(),
            stream_id: cfg.stream_id,
            rcv_buf_size: None,
            ignore_self_injected: true,
            ring_size: cfg.ring_size,
        };

        let mut tx = WfbTx::open(&tx_cfg).map_err(|e| Error::Io(format!("WfbTx::open: {e}")))?;
        let rx = WfbRx::open(&rx_cfg).map_err(|e| Error::Io(format!("WfbRx::open: {e}")))?;

        let (tx_in_s, tx_in_r) = bounded::<Vec<u8>>(256);
        let (rx_out_s, rx_out_r) = bounded::<RxFrame>(256);
        let shutdown = Arc::new(AtomicBool::new(false));

        // TX thread: blocks on queue; calls WfbTx::send.
        thread::Builder::new()
            .name("litm-tx".into())
            .spawn(move || {
                let mut seq: u32 = 1;
                while let Ok(bytes) = tx_in_r.recv() {
                    if let Err(e) = tx.send(&bytes, seq) {
                        warn!(error = ?e, "wfb_rs tx.send failed");
                    }
                    seq = seq.wrapping_add(1);
                }
                debug!("litm-tx exiting");
            })
            .map_err(|e| Error::Io(format!("spawn tx: {e}")))?;

        // RX thread: short-timeout loop; pushes to crossbeam channel.
        let shutdown_rx = shutdown.clone();
        thread::Builder::new()
            .name("litm-rx".into())
            .spawn(move || {
                let mut rx = rx;
                let mut buf = vec![0u8; 4096];
                while !shutdown_rx.load(Ordering::Relaxed) {
                    match rx.recv(&mut buf, Duration::from_millis(200)) {
                        Ok(Some((n, meta))) => {
                            let frame = RxFrame {
                                bytes: buf[..n].to_vec(),
                                rssi_dbm: meta.rssi[0],
                            };
                            if rx_out_s.try_send(frame).is_err() {
                                warn!("litm rx fanout queue full, dropping");
                            }
                        }
                        Ok(None) => continue,
                        Err(e) => {
                            error!(?e, "wfb_rs rx error");
                            break;
                        }
                    }
                }
                debug!("litm-rx exiting");
            })
            .map_err(|e| Error::Io(format!("spawn rx: {e}")))?;

        Ok(Self {
            tx_queue: tx_in_s,
            rx_queue: rx_out_r,
            cfg,
            shutdown,
        })
    }

    pub fn set_channel(&self, ch: u8) -> Result<()> {
        let out = Command::new("iw")
            .arg("dev")
            .arg(&self.cfg.iface)
            .arg("set")
            .arg("channel")
            .arg(ch.to_string())
            .output()
            .map_err(|e| Error::Io(format!("iw spawn: {e}")))?;
        if !out.status.success() {
            return Err(Error::Io(format!(
                "iw failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        Ok(())
    }
}

impl Drop for Radio {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}
