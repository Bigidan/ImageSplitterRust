// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
pub mod tools;

use crate::tools::image_process::ImageProcessor;
use image::GenericImageView;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct ImageSlice {
    path: String,
    width: u32,
    height: u32,
}

struct AppState {
    processor: tokio::sync::Mutex<Option<ImageProcessor>>,
}

#[tauri::command]
async fn load_images(
    chapter_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<ImageSlice, String> {
    let mut processor = ImageProcessor::new(&chapter_path);
    processor.load_images()?;

    let big_image = processor.big_image.as_ref().unwrap();
    let (width, height) = big_image.dimensions();

    *state.processor.lock().await = Some(processor);

    Ok(ImageSlice {
        path: format!("{}/tmp/combined.png", chapter_path),
        width,
        height,
    })
}

#[tauri::command]
async fn get_image_slices(
    chapter_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ImageSlice>, String> {
    let processor = state.processor.lock().await;
    let processor = processor.as_ref().ok_or("Images not loaded")?;

    let mut slices = Vec::new();
    let tmp_path = PathBuf::from(&chapter_path).join("tmp");

    for i in 0..processor.slices.len() {
        let slice_path = tmp_path.join(format!("{}.png", i + 1));
        if !slice_path.exists() {
            return Err(format!("Slice {} not found", i + 1));
        }

        let image = image::open(&slice_path)
            .map_err(|e| format!("Failed to open slice {}: {}", i + 1, e))?;

        let (width, height) = image.dimensions();

        slices.push(ImageSlice {
            path: slice_path.to_string_lossy().into_owned(),
            width,
            height,
        });
    }

    Ok(slices)
}

#[tauri::command]
async fn export_images(
    chapter_path: String,
    separators: Vec<u32>,
    extention: String,
) -> Result<(), String> {
    let mut processor = ImageProcessor::new(&chapter_path);
    processor.load_images()?;
    processor.export_slices(&separators, &extention)
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
