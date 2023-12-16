use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{self, Read};
use std::io::{Seek, Write};
use std::path::Path;

use crate::constant::OUT_PATH;
use crate::{my_error::CustomError, MyError};
use chalk_rs::Chalk;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use tokio::fs::{self, File};
use walkdir::{DirEntry, WalkDir};
use zip::result::ZipError;
use zip::write::FileOptions;
use zip::ZipArchive;

const METHOD_STORED: zip::CompressionMethod = zip::CompressionMethod::Stored;

pub fn validate_file_name(file_name: &str) -> Result<(), MyError> {
    if file_name.contains('\\')
        || ((file_name.len() >= 2
            && file_name.chars().nth(1) == Some(':')
            && file_name.chars().nth(0).unwrap().is_alphabetic())
            || file_name.starts_with('/'))
        || (file_name.split('/').any(|part| part == ".."))
    {
        Err(MyError::Custom(CustomError::new(&format!(
            "Invalid file name: {file_name}"
        ))))
    } else {
        // all good
        Ok(())
    }
}

pub fn decode_zip_filename(raw: &[u8]) -> Result<String, MyError> {
    let (mut encode, confidence, _) = chardet::detect(raw);

    encode = if encode == "" {
        println!(
            "{}",
            Chalk::new()
                .light_red()
                .string(&"chardet::detect gives empty encode, fallback to Shift_JIS")
        );
        String::from("Shift_JIS")
    } else {
        encode
    };

    let decoder =
          encoding_from_whatwg_label(chardet::charset2encoding(&encode)).ok_or_else(|| {
              MyError::Custom(CustomError::new(&format!(
                  "get decoder by encoding_from_whatwg_label failed, use encode: {encode} confidence:{confidence}"
              )))
          })?;

    let decoded_string = decoder
        .decode(&raw, DecoderTrap::Ignore)
        .ok()
        .ok_or_else(|| {
            MyError::Custom(CustomError::new(&format!(
                "decode failed, encode: {encode} confidence:{confidence}"
            )))
        })?;

    if encode != "ascii" && encode != "utf-8" {
        println!(
              "entry encode: {encode}, confidence: {confidence}, raw: {}, decoded: {}, GB18030: {}, ISO-2022-JP: {}, Shift_JIS: {}",
              Chalk::new()
                  .yellow()
                  .string(&String::from_utf8_lossy(raw)),
              Chalk::new().green().string(&decoded_string.clone()),
              Chalk::new()
                  .cyan()
                  .string(&encoding_from_whatwg_label("GB18030")
                  .unwrap_or(encoding_from_whatwg_label("UTF-8").unwrap())
                  .decode(&raw, DecoderTrap::Ignore)
                  .unwrap_or(String::from("?"))),
              Chalk::new()
                  .magenta()
                  .string(&encoding_from_whatwg_label("ISO-2022-JP")
                  .unwrap_or(encoding_from_whatwg_label("UTF-8").unwrap())
                  .decode(&raw, DecoderTrap::Ignore)
                  .unwrap_or(String::from("?"))),
              Chalk::new()
                  .blue()
                  .string(&encoding_from_whatwg_label("Shift_JIS")
                  .unwrap_or(encoding_from_whatwg_label("UTF-8").unwrap())
                  .decode(&raw, DecoderTrap::Ignore)
                  .unwrap_or(String::from("?")))
          );
    }

    return Ok(decoded_string);
}

pub fn get_file_ext_or_itself(filename: &str) -> String {
    Path::new(&filename).extension()
          // 在找不到扩展名的时候改变返回的值
          .unwrap_or_else(|| OsStr::new(&filename))
          .to_str()
          // 如果 OsStr 无法转化为合法的 UTF-8 字符串，使用原始的 decoded_entry_name
          .unwrap_or(&filename).to_owned()
}

pub async fn zip_dir(
    src_dir: &str,
    dst_file: &str,
    filter: Option<Box<dyn FnMut(&DirEntry) -> bool>>,
) -> Result<(), MyError> {
    if !Path::new(src_dir).is_dir() {
        return Err(MyError::Zip(ZipError::FileNotFound));
    }

    // dst dir not found
    if !Path::new(dst_file)
        .parent()
        .ok_or(MyError::Custom(CustomError::new(&format!(
            "cannot find parent dir from {dst_file}"
        ))))?
        .exists()
    {
        fs::create_dir_all(&dst_file).await?;
    }

    // dst file is already exist
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

pub fn pad_start(s: &str, width: usize, pad: char) -> String {
    let len = s.chars().count();
    if width > len {
        (0..width - len).map(|_| pad).collect::<String>() + s
    } else {
        s.to_string()
    }
}

pub fn pad_end(s: &str, width: usize, pad: char) -> String {
    let len = s.chars().count();
    if width > len {
        s.to_string() + &(0..width - len).map(|_| pad).collect::<String>()
    } else {
        s.to_string()
    }
}

pub async fn unzip(path: String) -> Result<(HashMap<String, u32>, String, String), MyError> {
    let mut ret: HashMap<String, u32> = HashMap::new();

    let reader = File::open(&path).await?;
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

        let decoded_entry_name = decode_zip_filename(entry_name)?;

        validate_file_name(decoded_entry_name.as_str())?;

        let out_path = tmp_dir.path().join(&decoded_entry_name);

        println!("out_path: {:?}", out_path);

        // is not dir
        if !decoded_entry_name.ends_with('/') {
            // statistic file ext name
            *ret.entry(get_file_ext_or_itself(&decoded_entry_name).to_string())
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

            // unzip file and write file
            // let buffer: Result<Vec<u8>, std::io::Error> = entry.bytes().collect();
            // let buffer = buffer?;
            // let slice: &[u8] = &a;
            // fs::write(out_path, buffer).await?;
            io::copy(&mut file, &mut out_file)?;
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
