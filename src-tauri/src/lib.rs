pub mod tools;

use tauri::AppHandle;
use serde::{
    Deserialize,
    Serialize
};

use crate::tools::image_process::ImageProcessor;


#[derive(Serialize, Deserialize)]
struct ImageSlice {
    index: usize,
    width: u32,
    height: u32,
    start_y: u32,
    end_y: u32,
}

#[derive(Serialize, Deserialize)]
struct LoadedImageInfo {
    total_width: u32,
    total_height: u32,
    slices: Vec<ImageSlice>,
    needs_slicing: bool, // true якщо зображення занадто велике для canvas
}

struct AppState {
    processor: tokio::sync::Mutex<Option<ImageProcessor>>,
}

#[tauri::command]
async fn load_images(
    chapter_path: String,
    state: tauri::State<'_, AppState>,
    app: AppHandle,
) -> Result<LoadedImageInfo, String> {
    let mut processor = ImageProcessor::new(&chapter_path);
    let image_data = processor.load_images(app)?;

    let needs_slicing = image_data.total_height > 32000 || image_data.total_width > 32000;

    let mut slices = Vec::new();
    for slice in &image_data.slices {
        slices.push(ImageSlice {
            index: slice.index,
            width: slice.width,
            height: slice.height,
            start_y: slice.start_y,
            end_y: slice.end_y,
        });
    }

    *state.processor.lock().await = Some(processor);

    Ok(LoadedImageInfo {
        total_width: image_data.total_width,
        total_height: image_data.total_height,
        slices,
        needs_slicing: needs_slicing,
    })
}


#[tauri::command]
async fn get_image_slice_bytes(
    slice_index: usize,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let processor = state.processor.lock().await;
    let processor = processor.as_ref().ok_or("Зображення не завантажено")?;

    let base64_data = processor.get_slice_as_base64(slice_index)?;
    Ok(format!("data:image/png;base64,{}", base64_data))
}

// Повертає slice як base64 string для прямого використання в HTML
#[tauri::command]
async fn get_image_slice_base64(
    slice_index: usize,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let processor = state.processor.lock().await;
    let processor = processor.as_ref().ok_or("Images not loaded")?;
    
    let base64_data = processor.get_slice_as_base64(slice_index)?;
    Ok(format!("data:image/png;base64,{}", base64_data))
}

#[tauri::command]
async fn save_slices_to_files(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let processor = state.processor.lock().await;
    let processor = processor.as_ref().ok_or("Зображення не завантажено")?;
    
    processor.save_slice_to_tmp()
}

#[tauri::command]
async fn get_image_slices(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ImageSlice>, String> {
    let processor = state.processor.lock().await;
    let processor = processor.as_ref().ok_or("Зображення не завантажено")?;

    // Спочатку зберігаємо slice як файли
    processor.save_slice_to_tmp()?;

    let mut slices = Vec::new();
    let image_data = processor.get_image_data();

    for slice in &image_data.slices {
        slices.push(ImageSlice {
            index: slice.index,
            width: slice.width,
            height: slice.height,
            start_y: slice.start_y,
            end_y: slice.end_y,
        });
    }

    Ok(slices)
}

#[tauri::command]
async fn export_images(
    state: tauri::State<'_, AppState>,
    separators: Vec<u32>,
    extention: String,
) -> Result<(), String> {
    let processor = state.processor.lock().await;
    let processor = processor.as_ref().ok_or("Зображення не завантажено")?;

    processor.export_slices(&separators, &extention)
}

#[tauri::command]
async fn get_full_image_bytes(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    let processor = state.processor.lock().await;
    let processor = processor.as_ref().ok_or("Зображення не завантажено")?;

    let image_data = processor.get_image_data();

    // Перевіряємо розмір
    if image_data.total_height > 32000 || image_data.total_width > 32000 {
        return Err("Зображення занадто велике для повної передачі. Використовуйте slice.".to_string());
    }
    
    // Якщо є тільки один slice, повертаємо його
    if image_data.slices.len() == 1 {
        processor.get_slice_as_bytes(0)
    } else {
        Err("Зображення було розділено на частини. Використовуйте get_image_slice_bytes.".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            processor: tokio::sync::Mutex::new(None),
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_images,
            export_images,
            get_image_slices,
            get_full_image_bytes,
            save_slices_to_files,
            get_image_slice_bytes,
            get_image_slice_base64,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
