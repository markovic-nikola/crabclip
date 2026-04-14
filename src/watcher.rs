use crate::config::Settings;
use crate::history::{ClipContent, ClipEntry};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use image::DynamicImage;
use std::io::Cursor;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

pub fn start_watcher(
    tx: mpsc::Sender<ClipEntry>,
    poll_interval: Duration,
    suppress: Arc<Mutex<Option<String>>>,
    settings: Arc<Mutex<Settings>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut last_text: Option<String> = None;
        let mut last_image_fingerprint: Option<(usize, Vec<u8>)> = None;

        loop {
            thread::sleep(poll_interval);

            let mut clipboard = match arboard::Clipboard::new() {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Check suppress flag (set when app itself copies something)
            let suppressed = suppress.lock().ok().and_then(|s| s.clone());

            // Try image first — browsers often put both text (URL) and image
            // data on the clipboard for "Copy Image", so check image before text.
            let show_images = settings.lock().map(|s| s.show_images).unwrap_or(true);
            if show_images {
            if let Ok(img_data) = clipboard.get_image() {
                let raw_size = img_data.width * img_data.height * 4;
                if raw_size > MAX_IMAGE_BYTES {
                    continue;
                }

                let fingerprint_bytes: Vec<u8> = img_data
                    .bytes
                    .iter()
                    .take(256)
                    .copied()
                    .collect();
                let fingerprint = (img_data.bytes.len(), fingerprint_bytes);

                if Some(&fingerprint) != last_image_fingerprint.as_ref() {
                    last_image_fingerprint = Some(fingerprint);
                    last_text = None;

                    let width = img_data.width as u32;
                    let height = img_data.height as u32;

                    if let Some(rgba) =
                        image::RgbaImage::from_raw(width, height, img_data.bytes.into_owned())
                    {
                        let mut png_bytes = Vec::new();
                        let dyn_img = DynamicImage::ImageRgba8(rgba);
                        if dyn_img
                            .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
                            .is_ok()
                        {
                            // Clear suppress since this is an image, not text
                            if let Ok(mut s) = suppress.lock() {
                                *s = None;
                            }

                            let png_base64 = STANDARD.encode(&png_bytes);
                            let entry = ClipEntry::new(ClipContent::Image {
                                png_base64,
                                width,
                                height,
                            });
                            if tx.send(entry).is_err() {
                                return;
                            }
                        }
                    }
                    continue;
                }
            }
            } // show_images

            // Try text
            if let Ok(text) = clipboard.get_text() {
                if !text.is_empty() && Some(&text) != last_text.as_ref() {
                    last_text = Some(text.clone());
                    last_image_fingerprint = None;

                    // Skip if this is a re-copy from the app
                    if suppressed.as_deref() == Some(&text) {
                        if let Ok(mut s) = suppress.lock() {
                            *s = None;
                        }
                        continue;
                    }

                    let entry = ClipEntry::new(ClipContent::Text(text));
                    if tx.send(entry).is_err() {
                        return; // Receiver dropped, shutting down
                    }
                }
            }
        }
    })
}
