use async_zip::Compression;
use std::collections::HashMap;
use std::io::{self, Read, Seek, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::sync::Mutex;
use walkdir::{DirEntry, WalkDir};
use zip::result::ZipError;
use zip::write::FileOptions;
use zip::ZipArchive;

use crate::constant::METHOD_STORED;
use crate::{helper, CustomError, MyError};

pub async fn zip_dir(
    src_dir: &str,
    dst_file: &str,
    filter: Option<Box<dyn FnMut(&DirEntry) -> bool>>,
) -> Result<(), MyError> {
    if !Path::new(src_dir).is_dir() {
        return Err(MyError::Zip(ZipError::FileNotFound));
    }

    // check if dst dir not found
    if !Path::new(dst_file)
        .parent()
        .ok_or(CustomError::new(&format!(
            "cannot find parent dir from {dst_file}"
        )))?
        .exists()
    {
        fs::create_dir_all(&dst_file).await?;
    }

    // check if dst file is already exist
    if Path::new(dst_file).exists() {
        return Err(MyError::Custom(CustomError::new(&format!(
            "dst_file {dst_file} is already exist"
        ))));
    }

    let file = File::create(Path::new(dst_file)).await?;

    let walkdir = WalkDir::new(src_dir);

    let it = walkdir.into_iter().filter_map(|e| e.ok());
    let mut it = it.filter(if let Some(filter) = filter {
        filter
    } else {
        Box::new(|_: &DirEntry| true)
    });

    zip_dir_inner(&mut it, src_dir, file.into_std().await, METHOD_STORED)?;

    Ok(())
}

pub async fn unzip(path: String) -> Result<(HashMap<String, u32>, String, String), MyError> {
    let reader = File::open(&path).await?;

    let tmp_dir = tempfile::tempdir()?;
    println!("make tmp_dir {:?}", tmp_dir.path());

    let ret = unzip_inner_async(reader, tmp_dir.path()).await?;
    // unzip_inner(reader, tmp_dir.path()).await?;

    let temp_path_str = tmp_dir
        .into_path()
        .as_os_str()
        .to_string_lossy()
        .into_owned();

    Ok((ret, temp_path_str, helper::get_out_zip_path(&path)?))
}

fn zip_dir_inner<T>(
    it: &mut dyn Iterator<Item = DirEntry>,
    prefix: &str,
    writer: T,
    method: zip::CompressionMethod,
) -> Result<(), MyError>
where
    T: Write + Seek,
{
    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default()
        .compression_method(method)
        .unix_permissions(0o644);

    for entry in it {
        let path = entry.path();
        let name = path
            .strip_prefix(Path::new(prefix))
            .ok()
            .ok_or(CustomError::new(&format!(
                "strip_prefix path {prefix} failed"
            )))?;

        // Write file or directory explicitly
        // Some unzip tools unzip files with directory paths correctly, some do not!
        if path.is_file() {
            zip.start_file(name.as_os_str().to_string_lossy(), options)?;
            let mut f = std::fs::File::open(path)?;

            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if !name.as_os_str().is_empty() {
            // Only if not root! Avoids path spec / warning
            // and map name conversion failed error on unzip
            zip.start_file(name.as_os_str().to_string_lossy(), options)?;
        }
    }

    zip.finish()?;

    Ok(())
}

async fn zip_dir_inner_async(
    it: &mut dyn Iterator<Item = DirEntry>,
    prefix: &str,
    mut file: File,
    method: Compression,
) -> Result<(), MyError> {
    use async_zip::tokio::write::ZipFileWriter;

    let mut writer = ZipFileWriter::with_tokio(&mut file);

    // for entry in it {
    //     let path = entry.path();
    //     let name = path
    //         .strip_prefix(Path::new(prefix))
    //         .ok()
    //         .ok_or(CustomError::new(&format!(
    //             "strip_prefix path {prefix} failed"
    //         )))?;

    //     // Write file or directory explicitly
    //     // Some unzip tools unzip files with directory paths correctly, some do not!
    //     if path.is_file() {
    //         zip.start_file(name.as_os_str().to_string_lossy(), options)?;
    //         let mut f = std::fs::File::open(path)?;

    //         let mut buffer = Vec::new();
    //         f.read_to_end(&mut buffer)?;
    //         zip.write_all(&buffer)?;
    //         buffer.clear();
    //     } else if !name.as_os_str().is_empty() {
    //         // Only if not root! Avoids path spec / warning
    //         // and map name conversion failed error on unzip
    //         zip.start_file(name.as_os_str().to_string_lossy(), options)?;
    //     }
    // }

    writer.close().await?;

    Ok(())
}

async fn unzip_inner(
    reader: std::fs::File,
    out_dir: &Path,
) -> Result<HashMap<String, u32>, MyError> {
    let mut ret: HashMap<String, u32> = HashMap::new();
    let mut zip = ZipArchive::new(reader)?;
    let zip_len = zip.len();

    for i in 0..zip_len {
        let time_other = std::time::Instant::now();
        let mut file = zip.by_index(i)?;
        let entry_name = file.name_raw();

        // let extra_data = encoding::label::encoding_from_whatwg_label("UTF-8")
        //     .unwrap()
        //     .decode(&file.extra_data(), encoding::DecoderTrap::Ignore)
        //     .unwrap();
        // println!("extra data: {extra_data}");

        let decoded_entry_name = helper::decode_zip_filename(entry_name)?;

        helper::validate_file_name(decoded_entry_name.as_str())?;

        let out_path = out_dir.join(&decoded_entry_name);
        // println!("[unzip_inner] unzip out_path: {:?}", out_path);

        // is not dir
        if !decoded_entry_name.ends_with('/') {
            // statistic file ext name
            *ret.entry(helper::get_file_ext_or_itself(&decoded_entry_name).to_string())
                .or_insert(0) += 1u32;

            // ensure path exist
            let out_path_parent =
                Path::new(&out_path)
                    .parent()
                    .ok_or(CustomError::new(&format!(
                        "cannot get parent of path {:?}",
                        &out_path
                    )))?;
            // println!("[unzip_inner] out_path_parent: {:?}", out_path_parent);
            if !out_path_parent.exists() {
                println!("[unzip_inner] mkdir -p {:?}", out_path_parent);
                fs::create_dir_all(out_path_parent).await?;
            }

            // create file fd
            let mut out_file = std::fs::File::create(&out_path)?;
            let time_other = time_other.elapsed().as_nanos() as f64 / 1000.0;
            let time = std::time::Instant::now();
            io::copy(&mut file, &mut out_file)?;
            println!(
                "[unzip_inner] copy {:?} -> {:?} ({:.2} ms unzip & I/O, {:.2} μs other)",
                decoded_entry_name,
                out_path,
                time.elapsed().as_micros() as f64 / 1000.0,
                time_other
            );
        } else {
            // mkdir dir from zip
            println!("[unzip_inner] mkdir -p {:?}", out_path);
            fs::create_dir_all(out_path).await?;
        }
    }

    Ok(ret)
}

async fn unzip_inner_async(archive: File, out_dir: &Path) -> Result<HashMap<String, u32>, MyError> {
    use async_zip::tokio::read::seek::ZipFileReader;
    use tokio::fs::OpenOptions;
    use tokio_util::compat::TokioAsyncWriteCompatExt;

    let ret: HashMap<String, u32> = HashMap::new();
    // let archive = archive.compat();
    let zip = ZipFileReader::with_tokio(archive).await?;

    let zip_arc = Arc::new(Mutex::new(zip));
    let ret_arc = Arc::new(Mutex::new(ret));

    let mut handles = vec![];

    for index in 0..zip_arc.lock().await.file().entries().len() {
        let out_dir = out_dir.to_owned();

        let zip_arc = Arc::clone(&zip_arc); // 克隆
        let ret_arc = Arc::clone(&ret_arc); // 克隆

        handles.push(tokio::spawn(async move {
            let mut locked_zip_arc = zip_arc.lock().await;

            match locked_zip_arc
                .file()
                .entries()
                .get(index)
                .ok_or(MyError::Custom(CustomError::new(&format!(
                    "async zip get index {index} failed"
                )))) {
                Ok(entry) => {
                    let filename = entry.entry().filename().as_bytes();
                    if let Ok(decoded_entry_name) = helper::decode_zip_filename(filename) {
                        match helper::validate_file_name(&decoded_entry_name) {
                            Ok(_) => {
                                let path = out_dir.join(&decoded_entry_name);

                                match locked_zip_arc.reader_with_entry(index).await {
                                    Ok(mut entry_reader) => {
                                        if decoded_entry_name.ends_with('/') {
                                            // The directory may have been created if iteration is out of order.
                                            if !path.exists() {
                                                match fs::create_dir_all(&path).await {
                                                    Err(e) => eprint!("{:?}", e),
                                                    _ => {}
                                                }
                                            }
                                        } else {
                                            // statistic file ext name
                                            let name_clone = String::from(&decoded_entry_name);
                                            *ret_arc
                                                .lock()
                                                .await
                                                .entry(
                                                    helper::get_file_ext_or_itself(&name_clone)
                                                        .to_string(),
                                                )
                                                .or_insert(0) += 1u32;

                                            // Creates parent directories. They may not exist if iteration is out of order
                                            // or the archive does not contain directory entries.
                                            let parent = path.parent().expect(
                                                "A file entry should have parent directories",
                                            );
                                            if !parent.is_dir() {
                                                match fs::create_dir_all(parent).await {
                                                    Err(e) => eprint!("{:?}", e),
                                                    _ => {}
                                                }
                                            }
                                            match OpenOptions::new()
                                                .write(true)
                                                .create_new(true)
                                                .open(&path)
                                                .await
                                            {
                                                Ok(writer) => {
                                                    match futures_util::io::copy(
                                                        &mut entry_reader,
                                                        &mut writer.compat_write(),
                                                    )
                                                    .await
                                                    {
                                                        Err(e) => eprint!("{:?}", e),
                                                        _ => {}
                                                    }
                                                }
                                                Err(e) => eprint!("{:?}", e),
                                            }
                                        }
                                    }
                                    Err(e) => eprint!("{:?}", e),
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => eprint!("{:?}", e),
            }
        }));
    }

    for handle in handles {
        match handle.await {
            Ok(_) => {}
            Err(e) => eprint!("{:?}", e),
        }
    }

    let result = {
        let ret_lock = ret_arc.lock().await;
        ret_lock.clone()
    };
    Ok(result)
}
