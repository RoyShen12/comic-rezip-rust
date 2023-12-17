use chalk_rs::Chalk;
use comic_rezip::constant::{TRANSFORM_EXT, TRASH_EXT};
use comic_rezip::zip;
use image_convert::{to_jpg, ImageResource, JPGConfig};
use std::env;
use std::io::ErrorKind;
use std::path::Path;
use tokio::fs::{self};
use walkdir::{DirEntry, WalkDir};

async fn process_zip_file(full_path: String) {
    println!("[async process_zip_file]({full_path}) entered");

    match zip::unzip(full_path.clone()).await {
        Ok((map, temp_path_str, dest_file)) => {
            println!(
                "[async process_zip_file]({full_path}) unzip {} Result",
                Chalk::new().bold().string(&full_path)
            );

            for (k, v) in &map {
                println!(
                    "{k} -> {} -> {v}",
                    mime_guess::from_ext(k.as_str())
                        .first_or_octet_stream()
                        .to_string()
                )
            }

            // transform some files
            let mut handles = vec![];
            // [transform] *.png, *.bmp, *.JPG, *.webm, *.webp to standard JPEG
            for entry in WalkDir::new(&temp_path_str)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let fd_path = entry.path().to_owned();
                if *&fd_path.is_file()
                    && (TRANSFORM_EXT.iter().any(|ext| -> bool {
                        fd_path
                            .extension()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .eq(ext)
                    }))
                {
                    if let Some(parent_path) = fd_path.parent() {
                        if let Some(file_base_name) = fd_path.file_stem() {
                            let target_path = Path::join(
                                parent_path,
                                String::from(file_base_name.to_string_lossy() + ".jpg"),
                            );
                            handles.push(tokio::spawn(async move {
                                let mut config = JPGConfig::new();
                                config.quality = 86;
                                let input = ImageResource::from_path(fd_path.to_path_buf());
                                let mut output = ImageResource::from_path(&target_path);
                                let time = std::time::Instant::now();
                                match to_jpg(&mut output, &input, &config) {
                                    Ok(_) => {
                                        // remove origin pic
                                        loop {
                                            match fs::remove_file(fd_path.to_path_buf()).await {
                                                Ok(_) => {
                                                    println!("[async process_zip_file] [async thread task] convert {:?} to {:?} ({} ms)", fd_path, target_path,time.elapsed().as_millis());
                                                    break;
                                                },
                                                Err(e) => match e.kind() {
                                                    ErrorKind::NotFound => break,
                                                    _ => {}
                                                },
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        eprint!("to_jpg src:{:?} error: {}", fd_path, err)
                                    }
                                }
                            }));
                        } else {
                            eprintln!("[transform] fd_path.file_stem error");
                        }
                    } else {
                        eprintln!("[transform] fd_path.parent error");
                    }
                }
            }

            for handle in handles {
                match handle.await {
                    Ok(_) => {}
                    Err(e) => eprint!("{:?}", e),
                }
            }

            // rezip dir
            match zip::zip_dir(
                &temp_path_str,
                &dest_file,
                Some(Box::new(|fd_entry: &DirEntry| -> bool {
                    let fd_path = fd_entry.path();
                    // [remove dir] __MACOSX, __MACOSX/*
                    if fd_path.is_dir() && fd_path.to_string_lossy().contains("__MACOSX") {
                        return false;
                    }
                    // [remove] *.url, *.db, *.txt
                    if fd_path.is_file()
                        && (TRASH_EXT.iter().any(|ext| -> bool {
                            fd_path
                                .extension()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .eq(ext)
                        }))
                    {
                        return false;
                    }
                    return true;
                })),
            )
            .await
            {
                Ok(_) => {}
                Err(e) => eprintln!("{e}"),
            }

            // clean temp dir
            match fs::remove_dir_all(&temp_path_str).await {
                Ok(_) => println!("clean tmp dir ok"),
                Err(e) => eprintln!("{e}"),
            }
        }
        Err(e) => {
            eprintln!(
                "unzip {} failed: {e}",
                Chalk::new().bold().string(&full_path)
            )
        }
    }
}

// scan_dir eat all errors
// let it panic
fn scan_dir(path: &str) {
    let mut handles = vec![];
    for (path, file_type) in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| (Path::new(path).join(e.file_name()), e.file_type()))
    {
        if let Some(full_path) = path.to_str() {
            let full_path = full_path.to_string();

            if file_type.is_file() && full_path.ends_with(".zip") {
                handles.push(std::thread::spawn(|| {
                    if let Ok(rt) = tokio::runtime::Runtime::new() {
                        let local_set = tokio::task::LocalSet::new();
                        local_set.spawn_local(process_zip_file(full_path));
                        rt.handle().block_on(async { local_set.await });
                    }
                }));
            }
        } else {
            eprintln!(
                "{}",
                Chalk::new()
                    .light_red()
                    .string(&format!("path {path:?} to string failed"))
            )
        }
    }
    for handle in handles {
        handle.join().unwrap();
    }
}

fn main() {
    let time = std::time::Instant::now();
    let args: Vec<String> = env::args().collect();
    // println!("{args:?}");
    let _ = scan_dir(&args[1]);
    println!(
        "{:.2} sec main fn",
        time.elapsed().as_millis() as f64 / 1000.0
    );
}
