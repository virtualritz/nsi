/// Juypiter Notebooks integration
pub trait EvcxrResult {
    fn evcxr_display(&self);
}

impl EvcxrResult for image::RgbImage {
    fn evcxr_display(&self) {
        let mut buffer = Vec::new();
        image::codecs::png::PNGEncoder::new(&mut buffer)
            .encode(&**self, self.width(), self.height(), image::ColorType::Rgb8)
            .unwrap();
        let img = base64::encode(&buffer);
        println!("EVCXR_BEGIN_CONTENT image/png\n{}\nEVCXR_END_CONTENT", img);
    }
}

impl EvcxrResult for image::RgbaImage {
    fn evcxr_display(&self) {
        let mut buffer = Vec::new();
        image::codecs::png::PNGEncoder::new(&mut buffer)
            .encode(
                &**self,
                self.width(),
                self.height(),
                image::ColorType::Rgba8,
            )
            .unwrap();
        let img = base64::encode(&buffer);
        println!("EVCXR_BEGIN_CONTENT image/png\n{}\nEVCXR_END_CONTENT", img);
    }
}

impl EvcxrResult for image::GrayImage {
    fn evcxr_display(&self) {
        let mut buffer = Vec::new();
        image::codecs::png::PNGEncoder::new(&mut buffer)
            .encode(&**self, self.width(), self.height(), image::ColorType::L8)
            .unwrap();
        let img = base64::encode(&buffer);
        println!("EVCXR_BEGIN_CONTENT image/png\n{}\nEVCXR_END_CONTENT", img);
    }
}

impl EvcxrResult for image::GrayAlphaImage {
    fn evcxr_display(&self) {
        let mut buffer = Vec::new();
        image::codecs::png::PNGEncoder::new(&mut buffer)
            .encode(&**self, self.width(), self.height(), image::ColorType::La8)
            .unwrap();
        let img = base64::encode(&buffer);
        println!("EVCXR_BEGIN_CONTENT image/png\n{}\nEVCXR_END_CONTENT", img);
    }
}
