use std::collections::HashMap;
use std::io::{self, Read};
use std::io::{Seek, Write};
use std::path::Path;
use tokio::fs::{self, File};
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

    let ret = unzip_inner(reader, tmp_dir.path()).await?;

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
) -> zip::result::ZipResult<()>
where
    T: Write + Seek,
{
    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default()
        .compression_method(method)
        .unix_permissions(0o644);

    let mut buffer = Vec::new();

    for entry in it {
        let path = entry.path();
        let name = path.strip_prefix(Path::new(prefix)).unwrap();

        // Write file or directory explicitly
        // Some unzip tools unzip files with directory paths correctly, some do not!
        if path.is_file() {
            // println!("adding file {path:?} as {name:?} ...");

            zip.start_file(name.as_os_str().to_string_lossy(), options)?;
            let mut f = std::fs::File::open(path)?;

            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if !name.as_os_str().is_empty() {
            // Only if not root! Avoids path spec / warning
            // and mapname conversion failed error on unzip
            // println!("adding dir {path:?} as {name:?} ...");

            zip.start_file(name.as_os_str().to_string_lossy(), options)?;
        }
    }
    zip.finish()?;
    Ok(())
}

async fn unzip_inner(reader: File, out_dir: &Path) -> Result<HashMap<String, u32>, MyError> {
    let mut ret: HashMap<String, u32> = HashMap::new();
    let mut zip = ZipArchive::new(reader.into_std().await)?;
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

        let out_path = out_dir.join(&decoded_entry_name);
        println!("unzip out_path: {:?}", out_path);

        // is not dir
        if !decoded_entry_name.ends_with('/') {
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
            let mut out_file = std::fs::File::create(&out_path)?;

            io::copy(&mut file, &mut out_file)?;
        } else {
            // mkdir dir from zip
            println!("mkdir -p {:?}", out_path);
            fs::create_dir_all(out_path).await?;
        }
    }

    Ok(ret)
}

// async fn unzip_inner_async(archive: File, out_dir: &Path) -> Result<(), MyError> {
//     use async_zip::tokio::read::seek::ZipFileReader;
//     use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
//
//     let archive = archive.compat();
//     let mut reader = ZipFileReader::new(archive).await?;

//     for index in 0..reader.file().entries().len() {
//         let entry = reader
//             .file()
//             .entries()
//             .get(index)
//             .ok_or(MyError::Custom(CustomError::new(&format!(
//                 "async zip get index {index} failed"
//             ))))?;

//         let filename = entry.entry().filename();
//         validate_file_name(filename);
//         let path = out_dir.join(filename);

//         // If the filename of the entry ends with '/', it is treated as a directory.
//         // This is implemented by previous versions of this crate and the Python Standard Library.
//         // https://docs.rs/async_zip/0.0.8/src/async_zip/read/mod.rs.html#63-65
//         // https://github.com/python/cpython/blob/820ef62833bd2d84a141adedd9a05998595d6b6d/Lib/zipfile.py#L528
//         let entry_is_dir = entry.entry().dir();

//         // let mut entry_reader = reader.reader_without_entry(index).await?;

//         if entry_is_dir {
//             // The directory may have been created if iteration is out of order.
//             if !path.exists() {
//                 fs::create_dir_all(&path).await?;
//             }
//         } else {
//             // Creates parent directories. They may not exist if iteration is out of order
//             // or the archive does not contain directory entries.
//             let parent = path
//                 .parent()
//                 .expect("A file entry should have parent directories");
//             if !parent.is_dir() {
//                 fs::create_dir_all(parent).await?;
//             }
//             let writer = OpenOptions::new()
//                 .write(true)
//                 .create_new(true)
//                 .open(&path)
//                 .await?;
//             futures_util::io::copy(&mut entry_reader, &mut writer.compat_write()).await?;

//             // Closes the file and manipulates its metadata here if you wish to preserve its metadata from the archive.
//         }
//     }

//     Ok(())
// }
