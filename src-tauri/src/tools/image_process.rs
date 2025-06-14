use std::{
    fs,
    path::Path
};
use serde::{
    Deserialize,
    Serialize
};
use image::{
    ImageFormat,
    ImageBuffer,
    DynamicImage,
    GenericImage,
    GenericImageView,
};

use natord::compare;
use tauri::{AppHandle, Emitter};
use base64::{engine::general_purpose, Engine as _};

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    percentage: f64,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSlice {
    pub index: usize,
    pub start_y: u32,
    pub end_y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub total_width: u32,
    pub total_height: u32,
    pub slices: Vec<ImageSlice>,
}

pub struct ImageProcessor {
    pub big_image: Option<DynamicImage>,
    pub image_data: ImageData,
    pub chapter_path: String,
    pub max_slice_height: u32,
}

impl ImageProcessor {
    pub fn new(chapter_path: &str) -> Self {
        Self {
            big_image: None,
            image_data: ImageData {
                total_width: 0,
                total_height: 0,
                slices: Vec::new(),
            },
            chapter_path: chapter_path.to_string(),
            max_slice_height: 12000,
        }
    }

    pub fn set_max_slice_height(&mut self, height: u32) {
        self.max_slice_height = height;
    }

    fn try_load_cached_slices(&mut self) -> Option<ImageData> {
        let tmp_path = Path::new(&self.chapter_path).join("tmp");

        if !tmp_path.exists() {
            return None;
        }

        let mut slices = Vec::new();
        let mut images = Vec::new();
        let mut total_height = 0;
        let mut width = 0;
        let mut index = 1;

        loop {
            let slice_path = tmp_path.join(format!("{}.png", index));
            if !slice_path.exists() {
                break;
            }

            let img = image::open(&slice_path).ok()?.to_rgba8();
            let (w, h) = img.dimensions();

            if width == 0 {
                width = w;
            } else if width != w {
                return None; // неузгоджені ширини
            }

            slices.push(ImageSlice {
                index,
                start_y: total_height,
                end_y: total_height + h,
                width: w,
                height: h,
            });

            images.push(img);
            total_height += h;
            index += 1;
        }

        if slices.is_empty() {
            return None;
        }

        // Відтворення big_image
        let mut big_image = ImageBuffer::new(width, total_height);
        let mut y_offset = 0;
        for img in images {
            big_image
                .copy_from(&img, 0, y_offset)
                .ok()?; // Якщо копіювання провалилось
            y_offset += img.height();
        }

        self.big_image = Some(DynamicImage::ImageRgba8(big_image));

        Some(ImageData {
            total_width: width,
            total_height,
            slices,
        })
    }
    
    pub fn load_images(&mut self, app: AppHandle) -> Result<ImageData, String> {

        if let  Some(cached) = self.try_load_cached_slices() {
            self.image_data = cached;
            return Ok(self.image_data.clone());
        }

        let raw_path = Path::new(&self.chapter_path).join("Raw");
        let mut files: Vec<_> = fs::read_dir(&raw_path)
            .map_err(|e| format!("Помилка читання теки: {}", e))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                if let Some(ext) = entry.path().extension() {
                    matches!(ext.to_str().unwrap_or("").to_lowercase().as_str(),
                            "png" | "jpg" | "jpeg" | "webp" | "bmp")
                }
                else {
                    false
                }
            })
            .collect();

        files.sort_by(|a, b| {
            let a_os = a.file_name();
            let b_os = b.file_name();

            let a_name = a_os.to_string_lossy();
            let b_name = b_os.to_string_lossy();
            compare(&a_name, &b_name)
        });

        // files.sort_by_key(|entry| entry.file_name().to_string_lossy().into_owned());

        let mut total_height = 0;
        let mut width = 0;
        let mut images: Vec<DynamicImage> = Vec::new();

        let total_entrys = files.len();
        for (i, entry) in files.iter().enumerate() {
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

            // Прогрес
            let percent = ((i + 1) as f64 / total_entrys as f64) * 100.0;
            let message = format!("Обробка зображення {}/{}", i + 1, total_entrys);
            app.emit("progress", ProgressPayload {
                percentage: percent * 0.48,
                message,
            }).unwrap();
        }

        if images.is_empty() {
            return Err("Не знайдено жодного зображення в папці Raw".to_string());
        }
        
        // Створюємо велике зображення
        let mut big_image = ImageBuffer::new(width, total_height);
        let mut y_offset = 0;

        for image in images {
            let sub_image = image.to_rgba8();
            big_image.copy_from(&sub_image, 0, y_offset)
                .map_err(|e| format!("Помилка копіювання: {}", e))?;
            y_offset += sub_image.height();
        }

        self.big_image = Some(DynamicImage::ImageRgba8(big_image));
        self.image_data.total_width = width;
        self.image_data.total_height = total_height;

        self.calculate_slices();

        app.emit("progress", ProgressPayload {
            percentage: 50.00,
            message: "Зображення об'єднано.".to_string(),
        }).unwrap();

        Ok(self.image_data.clone())
    }

    fn calculate_slices(&mut self) {
        let height = self.image_data.total_height;

        if height < self.max_slice_height {
            self.image_data.slices = vec![ImageSlice {
                index: 0,
                start_y: 0,
                end_y: height,
                width: self.image_data.total_width,
                height,
            }];
            return;
        }

        let num_slices = (height as f32 / self.max_slice_height as f32).ceil() as u32;
        let slice_height = height / num_slices;

        self.image_data.slices.clear();

        for i in 0..num_slices {
            let start_y = i * slice_height;
            let end_y = if i == num_slices - 1 {
                height
            } else {
                (i + 1) * slice_height
            };

            self.image_data.slices.push(ImageSlice {
                index: i as usize,
                start_y,
                end_y,
                width: self.image_data.total_width,
                height: end_y - start_y,
            });
        }
    }


    // Отримання в байтовому форматі
    pub fn get_slice_as_bytes(&self, slice_index: usize) -> Result<Vec<u8>, String> {
        let big_image = self.big_image.as_ref().ok_or("Немає великого зображення")?;

        if slice_index >= self.image_data.slices.len() {
            return Err("Неправильний індекс".to_string());
        }

        let slice_info = &self.image_data.slices[slice_index];
        let slice = big_image.crop_imm(
            0,
            slice_info.start_y,
            slice_info.width,
            slice_info.height
        );

        let mut bytes: Vec<u8> = Vec::new();
        slice
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .map_err(|e| format!("Помилка конвертації в bytes: {}", e))?;

        Ok(bytes)
    }

    pub fn get_slice_as_base64(&self, slice_index: usize) -> Result<String, String> {
        let bytes = self.get_slice_as_bytes(slice_index)?;
        Ok(general_purpose::STANDARD.encode(&bytes))
    }


    // Зберігання зображень
    pub fn save_slice_to_tmp(&self) -> Result<Vec<String>, String> {
        let big_image = self.big_image.as_ref().ok_or("Немає великого зображення")?;

        let tmp_path = Path::new(&self.chapter_path).join("tmp");
        fs::create_dir_all(&tmp_path)
            .map_err(|e| format!("Помилка створення tmp директорії: {}", e))?;

        let mut file_paths = Vec::new();

        for slice_info in &self.image_data.slices {
            let slice = big_image.crop_imm(
                0,
                slice_info.end_y,
                slice_info.width,
                slice_info.height,
            );

            let slice_path = tmp_path.join(format!("{}.png", slice_info.index));
            slice
                .save(&slice_path)
                .map_err(|e| format!("Помилка збереження slice: {}", e))?;

            file_paths.push(slice_path.to_string_lossy().to_string());
        }

        Ok(file_paths)
    }

    pub fn get_image_data(&self) -> &ImageData {
        &self.image_data
    }


    pub fn export_slices(&self, separators: &[u32], file_extantion: &str) -> Result<(), String> {
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
            let output_file = split_path.join(format!("{}.{}", index, file_extantion));
            part.save(&output_file)
                .map_err(|e| format!("Помилка зберігання частини {}.{}", index, e))?;

            start_y = end_y;
            index += 1;
        }

        if start_y < height {
            let part = big_image.crop_imm(0, start_y, width, height - start_y);
            let output_file = split_path.join(format!("{}.{}", index, file_extantion));
            part.save(&output_file)
                .map_err(|e| format!("Помилка зберігання частини {}.{}", index, e))?;
        }

        Ok(())
    }
}
