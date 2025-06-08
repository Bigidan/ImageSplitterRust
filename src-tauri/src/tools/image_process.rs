use std::{fs, path::Path};

use image::{DynamicImage, GenericImageView, ImageBuffer};

pub struct ImageProcessor {
    pub big_image: Option<DynamicImage>,
    pub slices: Vec<DynamicImage>,
    pub chapter_path: String,
}

impl ImageProcessor {
    pub fn new(chapter_path: &str) -> Self {
        Self {
            big_image: None,
            slices: Vec::new(),
            chapter_path: chapter_path.to_string(),
        }
    }

    pub fn load_images(&mut self) -> Result<(u32, u32), String> {
        let raw_path = Path::new(&self.chapter_path).join("Raw");
        let files: Vec<_> = fs::read_dir(&raw_path)
            .map_err(|e| format!("Помилка читання теки: {}", e))?
            .filter_map(|entry| entry.ok())
            .collect();

        //files.sort_by(|a, b| natsort::compare(&a.file_name(), &b.file_name()));

        let mut total_height = 0;
        let mut width = 0;
        let mut images: Vec<DynamicImage> = Vec::new();

        for entry in files {
            let image = image::open(entry.path())
                .map_err(|e| format!("Помилка відкриття зображення: {}", e))?;

            let (w, h) = image.dimensions();
            if width == 0 {
                width = w;
            } else if w != width {
                return Err("Усі зображення в папці повинні мати однакову ширину.".to_string());
            }

            total_height += h;
            images.push(image);
        }

        let mut big_image = ImageBuffer::new(width, total_height);
        let mut y_offset = 0;

        for image in images {
            let (w, h) = image.dimensions();
            for y in 0..h {
                for x in 0..w {
                    big_image.put_pixel(x, y + y_offset, image.get_pixel(x, y));
                }
            }
            y_offset += h;
        }

        self.big_image = Some(DynamicImage::ImageRgba8(big_image));
        self.slice_big_image()?;

        Ok((width, total_height))
    }

    fn slice_big_image(&mut self) -> Result<(), String> {
        let big_image = self.big_image.as_ref().ok_or("No big image loaded")?;
        let (width, height) = big_image.dimensions();

        let tmp_path = Path::new(&self.chapter_path).join("tmp");
        fs::create_dir_all(&tmp_path)
            .map_err(|e| format!("Failed to create tmp directory: {}", e))?;

        let num_slices = (height as f32 / 12000.0).ceil() as u32;
        let slice_height = height / num_slices;

        self.slices.clear();

        for i in 0..num_slices {
            let start_y = i * slice_height;
            let end_y = if i == num_slices - 1 {
                height
            } else {
                (i + 1) * slice_height
            };

            let slice = big_image.crop_imm(0, start_y, width, end_y - start_y);
            self.slices.push(slice.clone());

            let slice_path = tmp_path.join(format!("{}.png", i + 1));
            slice
                .save(&slice_path)
                .map_err(|e| format!("Failed to save slice: {}", e))?;
        }

        Ok(())
    }

    pub fn export_slices(&self, separators: &[u32], output_path: &str) -> Result<(), String> {
        let big_image = self.big_image.as_ref().ok_or("Немає великого зображення")?;
        let (width, height) = big_image.dimensions();

        let split_path = Path::new(&self.chapter_path).join("Split");
        fs::create_dir_all(&split_path)
            .map_err(|e| format!("Помилка створення теки для експорту: {}", e))?;

        let mut start_y = 0;
        let mut index = 1;
        for &separator in separators {
            let end_y = separator.min(height);
            let part = big_image.crop_imm(0, start_y, width, end_y - start_y);
            let output_file = split_path.join(format!("{}.{}", index, output_path));
            part.save(&output_file)
                .map_err(|e| format!("Помилка зберігання частини {}.{}", index, e))?;

            start_y = end_y;
            index += 1;
        }

        if start_y < height {
            let part = big_image.crop_imm(0, start_y, width, height - start_y);
            let output_file = split_path.join(format!("{}.{}", index, output_path));
            part.save(&output_file)
                .map_err(|e| format!("Помилка зберігання частини {}.{}", index, e))?;
        }

        Ok(())
    }
}
