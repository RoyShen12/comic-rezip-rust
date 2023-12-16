use std::ffi::OsStr;
use std::path::Path;

use crate::constant::{ASCII, FALLBACK_ENCODING, OUT_PATH, UTF8};
use crate::{my_error::CustomError, MyError};
use chalk_rs::Chalk;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};

pub fn validate_file_name(file_name: &str) -> Result<(), MyError> {
    if file_name.contains('\\')
        || ((file_name.len() >= 2
            && file_name.chars().nth(1) == Some(':')
            && file_name.chars().nth(0).unwrap().is_alphabetic())
            || file_name.starts_with('/'))
        || (file_name.split('/').any(|part| part == ".."))
    {
        Err(MyError::from(CustomError::new(&format!(
            "Invalid file name: {file_name}"
        ))))
    } else {
        // all good
        Ok(())
    }
}

pub fn get_out_zip_path(origin_filename: &str) -> Result<String, MyError> {
    Ok(dirs_next::home_dir()
        .ok_or(CustomError::new("cannot get home_dir"))?
        .join(Path::new(OUT_PATH))
        .join(
            Path::new(&origin_filename)
                .file_name()
                .ok_or(CustomError::new(&format!(
                    "cannot get file_name from {origin_filename}"
                )))?,
        )
        .to_string_lossy()
        .into_owned())
}

pub fn decode_zip_filename(raw: &[u8]) -> Result<String, MyError> {
    let (mut encode, confidence, _) = chardet::detect(raw);

    encode = if encode == "" {
        println!(
            "{}",
            Chalk::new().light_red().string(
                &(String::from("chardet::detect gives empty encode, fallback to ")
                    + FALLBACK_ENCODING)
            )
        );
        String::from(FALLBACK_ENCODING)
    } else {
        encode
    };

    let decoder =
          encoding_from_whatwg_label(chardet::charset2encoding(&encode)).ok_or_else(|| {
              CustomError::new(&format!(
                  "get decoder by encoding_from_whatwg_label failed, use encode: {encode} confidence:{confidence}"
              ))
          })?;

    let decoded_string = decoder
        .decode(&raw, DecoderTrap::Ignore)
        .ok()
        .ok_or_else(|| {
            CustomError::new(&format!(
                "decode failed, encode: {encode} confidence:{confidence}"
            ))
        })?;

    if encode != ASCII && encode != UTF8 {
        // encoding debug
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
          // 在找不到扩展名的时候返回本身的值
          .unwrap_or_else(|| OsStr::new(&filename))
          .to_str()
          // 如果 OsStr 无法转化为合法的 UTF-8 字符串，使用原始的 decoded_entry_name
          .unwrap_or(&filename).to_owned()
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
