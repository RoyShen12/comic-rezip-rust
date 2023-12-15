use chalk_rs::Chalk;
use comic_rezip::constant::{OUT_PATH, TRANSFORM_EXT, TRASH_EXT};
use comic_rezip::{helper, CustomError, MyError};
use image_convert::{to_jpg, ImageResource, JPGConfig};
use std::io;
use std::path::Path;
use std::{collections::HashMap, env};
use tokio::fs::{self, File};
use walkdir::{DirEntry, WalkDir};
use zip::ZipArchive;

async fn unzip(path: &str) -> Result<(HashMap<String, u32>, String, String), MyError> {
    let mut ret: HashMap<String, u32> = HashMap::new();

    let reader = File::open(path).await?;
    let mut zip = ZipArchive::new(reader.into_std().await)?;

    let tmp_dir = tempfile::tempdir()?;
    println!("make tmp_dir {:?}", tmp_dir.path());

    let zip_len = zip.len();
    for i in 0..zip_len {
        let mut file = zip.by_index(i)?;
        let entry_name = file.name_raw();

        // let extra_data = encoding::label::encoding_from_whatwg_label("UTF-8")
        //     .unwrap()
        //     .decode(&file.extra_data(), encoding::DecoderTrap::Ignore)
        //     .unwrap();
        // println!("extra data: {extra_data}");

        let decoded_entry_name = helper::decode_zip_filename(entry_name)?;

        helper::validate_file_name(decoded_entry_name.as_str())?;

        let out_path = tmp_dir.path().join(&decoded_entry_name);

        println!("out_path: {:?}", out_path);

        if !file.is_dir() {
            // statistic file ext name
            *ret.entry(helper::get_file_ext_or_itself(&decoded_entry_name).to_string())
                .or_insert(0) += 1u32;

            // ensure path exist
            let out_path_parent = Path::new(&out_path).parent();
            println!("out_path_parent: {:?}", out_path_parent);
            if out_path_parent.is_some() && !out_path_parent.unwrap().exists() {
                println!("mkdir -p {:?}", out_path_parent);
                fs::create_dir_all(out_path_parent.unwrap()).await?;
            }

            // create file fd
            println!("create file fd: {:?}", out_path);
            let out_file = File::create(&out_path).await?;

            // unzip file
            io::copy(&mut file, &mut out_file.into_std().await)?;
        } else {
            // mkdir dir from zip
            println!("mkdir -p {:?}", out_path);
            fs::create_dir_all(out_path).await?;
        }
    }

    let dest_file = dirs_next::home_dir()
        .ok_or(MyError::Custom(CustomError::new("cannot get home_dir")))?
        .join(Path::new(OUT_PATH))
        .join(
            Path::new(&path)
                .file_name()
                .ok_or(MyError::Custom(CustomError::new(&format!(
                    "cannot get file_name from {path}"
                ))))?,
        )
        .to_string_lossy()
        .into_owned();
    let temp_path_str = tmp_dir
        .into_path()
        .as_os_str()
        .to_string_lossy()
        .into_owned();

    Ok((ret, temp_path_str, dest_file))
}

// scan_dir eat all errors
// let it panic
async fn scan_dir(path: &str) {
    // println!("goto path: {:?}", path);

    for (path, file_type) in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| (Path::new(path).join(e.file_name()), e.file_type()))
    {
        if let Some(full_path) = path.to_str() {
            let full_path = full_path.to_string();

            if file_type.is_file() && full_path.ends_with(".zip") {
                let full_path = full_path.clone();
                match unzip(&full_path).await {
                    Ok((map, temp_path_str, dest_file)) => {
                        println!("unzip {} Result", Chalk::new().bold().string(&full_path));

                        for (k, v) in &map {
                            println!(
                                "{k} -> {} -> {v}",
                                mime_guess::from_ext(k.as_str())
                                    .first_or_octet_stream()
                                    .to_string()
                            )
                        }

                        // transform some files
                        // [transform] *.png, *.bmp, *.JPG, *.webm, *.webp to standard JPEG
                        for entry in WalkDir::new(&temp_path_str)
                            .into_iter()
                            .filter_map(|e| e.ok())
                        {
                            let fd_path = entry.path();
                            println!(
                                "for entry in WalkDir::new(&temp_path_str) @ {:?} is file {}",
                                entry,
                                fd_path.is_file()
                            );
                            if fd_path.is_file()
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

                                        let mut config = JPGConfig::new();
                                        config.quality = 86;
                                        let input = ImageResource::from_path(fd_path.to_path_buf());
                                        let mut output = ImageResource::from_path(&target_path);
                                        println!("convert {:?} to {:?}", fd_path, target_path);
                                        match to_jpg(&mut output, &input, &config) {
                                            Ok(_) => {}
                                            Err(err) => {
                                                eprint!("to_jpg src:{:?} error: {}", fd_path, err)
                                            }
                                        }
                                    } else {
                                        eprintln!("[transform] fd_path.file_stem error");
                                    }
                                } else {
                                    eprintln!("[transform] fd_path.parent error");
                                }
                            }
                        }

                        // rezip dir
                        match helper::zip_dir(
                            &temp_path_str,
                            &dest_file,
                            Some(Box::new(|fd_entry: &DirEntry| -> bool {
                                let fd_path = fd_entry.path();
                                // [remove dir] __MACOSX, __MACOSX/*
                                if fd_path.is_dir()
                                    && fd_path.to_string_lossy().contains("__MACOSX")
                                {
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
                        ) {
                            Ok(()) => {}
                            Err(e) => eprintln!("{e}"),
                        }

                        // clean temp dir
                        match fs::remove_dir_all(&temp_path_str).await {
                            Ok(()) => println!("clean tmp dir ok"),
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
        } else {
            eprintln!(
                "{}",
                Chalk::new()
                    .light_red()
                    .string(&format!("path {path:?} to string failed"))
            )
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    // println!("{args:?}");
    let _ = scan_dir(&args[1]).await;
}
