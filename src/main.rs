use comic_rezip::helper;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use std::path::Path;
use std::{collections::HashMap, env};
// use std::sync::Arc;
use chalk_rs::Chalk;
use std::fs::{self, File};
use std::io::{Error, ErrorKind};
use zip::ZipArchive;

// async fn print_dir<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
//     let mut entries = fs::read_dir(path).await?;

//     while let Some(entry) = entries.next_entry().await? {
//         println!("{}", entry.file_name().to_string_lossy());
//     }

//     Ok(())
// }

// struct User {
//     active: bool,
//     username: String,
//     email: Option<String>,
//     sign_in_count: u64,
// }

fn unzip(path: &str) -> Result<HashMap<String, u32>, Error> {
    let meta = fs::metadata(path).ok().ok_or_else(|| {
        Error::new(
            ErrorKind::Other,
            Chalk::new()
                .light_red()
                .string(&format!("cannot get file meta of {} !", path)),
        )
    })?;

    println!(
        "check file: ({:.1} MB) {:?}",
        meta.len() as f64 / 1024.0 / 1024.0,
        path,
    );

    let mut ret: HashMap<String, u32> = HashMap::new();

    let reader = File::open(path)?;
    let mut zip = ZipArchive::new(reader)?;

    let zip_len = zip.len();
    for i in 0..zip_len {
        let file = zip.by_index(i)?;
        let entry_name = file.name_raw();
        let (encode, confidence, _) = chardet::detect(entry_name);

        let coder =
            encoding_from_whatwg_label(chardet::charset2encoding(&encode)).ok_or_else(|| {
                Error::new(
                    ErrorKind::Other,
                    Chalk::new()
                        .light_red()
                        .string(&"encoding_from_whatwg_label failed"),
                )
            })?;
        let decoded_entry_name = coder
            .decode(&entry_name, DecoderTrap::Ignore)
            .ok()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Other,
                    Chalk::new().light_red().string(&"decode failed"),
                )
            })?;

        if !helper::validate_file_name(decoded_entry_name.as_str()) {
            return Err(Error::new(
                ErrorKind::Other,
                Chalk::new().light_red().string(&"Invalid file name"),
            ));
        }

        let is_dir = file.is_dir();

        if encode != "ascii" && encode != "utf-8" {
            println!(
                "entry<{}/{}> [{}] encode: {}, confidence: {}, raw: {}, decoded: {}",
                i,
                zip_len,
                if is_dir {
                    Chalk::new().green().string(&"D")
                } else {
                    Chalk::new().yellow().string(&"F")
                },
                encode,
                confidence,
                Chalk::new()
                    .yellow()
                    .string(&String::from_utf8_lossy(entry_name)),
                Chalk::new().green().string(&decoded_entry_name.clone())
            );
        }

        if !is_dir {
            let ext = Path::new(&decoded_entry_name)
                .extension()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::Other,
                        Chalk::new().light_red().string(&format!(
                            "get path \"{}\" extension failed",
                            decoded_entry_name
                        )),
                    )
                })?
                .to_str()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::Other,
                        Chalk::new()
                            .light_red()
                            .string(&format!("ext to str (path:{}) failed", decoded_entry_name)),
                    )
                })?;
            *ret.entry(ext.to_string()).or_insert(0) += 1u32
        }
    }
    Ok(ret)
}

fn scan_dir(path: &str) -> Result<(), Error> {
    println!("goto path: {:?}", path);

    let dir = fs::read_dir(path).ok().ok_or_else(|| {
        Error::new(
            ErrorKind::Other,
            Chalk::new().light_red().string(&"fs::read_dir failed"),
        )
    })?;

    for entry in dir {
        let ok_entry = entry.ok().ok_or_else(|| {
            Error::new(
                ErrorKind::Other,
                Chalk::new().light_red().string(&"get entry failed"),
            )
        })?;

        let path = Path::new(path).join(ok_entry.file_name());
        let full_path = path.to_str().ok_or_else(|| {
            Error::new(
                ErrorKind::Other,
                Chalk::new()
                    .light_red()
                    .string(&format!("path {:?} to string failed", path)),
            )
        })?;

        let file_type = ok_entry.file_type().ok().ok_or_else(|| {
            Error::new(
                ErrorKind::Other,
                Chalk::new()
                    .light_red()
                    .string(&"get entry file_type failed"),
            )
        })?;

        if file_type.is_dir() && !full_path.contains("node_modules") {
            let _ = scan_dir(full_path);
        } else if file_type.is_file() && full_path.ends_with(".zip") {
            match unzip(full_path) {
                Ok(map) => println!("\nunzip Result: {:?}\n", map),
                Err(e) => println!("{}", e.to_string()),
            }
        }
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);
    let _ = scan_dir(&args[1]);

    // let user = User {
    //     email: Some(String::from("someone@example.com")),
    //     username: String::from("someusername123"),
    //     active: true,
    //     sign_in_count: 1,
    // };
    // let yes = if user.email.is_some() { "1" } else { "2" };
    // let rt = Runtime::new().unwrap();
    // rt.block_on(print_dir("/Users/royshen/WorkRepo"));

    // let reader = File::open("/Users/royshen/Downloads/test/test.zip");
    // let reader = match reader {
    //     Ok(file) => file,
    //     Err(error) => {
    //         panic!("Problem opening the file: {:?}", error)
    //     }
    // };

    // let mut zip = ZipArchive::new(reader).unwrap();

    // for i in 0..zip.len() {
    //     let file = zip.by_index(i).unwrap();
    //     let file_name = file.name().as_bytes();

    //     let det = detect(file_name); // 使用chardet获取编码和置信度
    //     let encoding_label = &det.0;

    //     // 使用encoding_rs的for_label函数获取编码
    //     let encoding = Encoding::for_label(encoding_label.as_bytes()).unwrap_or(UTF_8); // 使用UTF_8作为备用方案
    //     println!("encoding.name: {}", encoding.name());
    //     let (decoded, _, had_errors) = encoding.decode(file_name);

    //     if had_errors {
    //         println!("解码错误!");
    //     } else {
    //         println!("文件名：{}", decoded);
    //     }
    // }
}
