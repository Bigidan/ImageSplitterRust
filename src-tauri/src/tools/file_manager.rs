use std::fs::read_dir;

pub fn count_images_numb(chapter_path: &String, images_folder: Option<String>) -> usize {
    let full_path = format!(
        "{}/{}/",
        chapter_path,
        images_folder.unwrap_or("tmp".to_string())
    );

    let paths = match read_dir(&full_path) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("Помилка читання теки {}: {}", full_path, e);
            return 0;
        }
    };

    // for path in paths {
    //     println!("Name: {}", path.unwrap().path().display())
    // }
    paths.count()
}
