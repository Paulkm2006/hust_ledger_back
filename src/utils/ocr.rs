use image::codecs::gif::GifDecoder;
use actix_web::web::Bytes;
use std::io::Cursor;
use image::{DynamicImage, AnimationDecoder};

pub async fn decode_captcha(img: Bytes) -> Result<String, Box<dyn std::error::Error>> {
	let g: GifDecoder<Cursor<Bytes>> = GifDecoder::new(Cursor::new(img)).map_err(|e| format!("Failed to create GifDecoder: {}", e))?;
	let frames: Vec<_> = g.into_frames().collect::<Result<_, _>>()?;
	let grayscale_frames: Vec<_> = frames.iter().map(|f| DynamicImage::ImageRgba8(f.buffer().clone()).into_luma8()).collect();
	let mut img: image::ImageBuffer<image::Luma<u8>, Vec<u8>> = image::ImageBuffer::new(90, 58);
	for w in 0..90 {
		for h in 0..58 {
			let mut cnt = 0;
			for frame in &grayscale_frames {
				let pixel = frame.get_pixel(w, h);
				if pixel[0] != 255 {
					cnt += 1;
				}
			}
			if cnt >= 3 {
				img.put_pixel(w, h, image::Luma([255u8]));
			}
		}
	}
	let text = tesseract::ocr_from_frame(img.into_raw().as_slice(), 90, 58, 1, 90, "eng")?;
	Ok(text)
}