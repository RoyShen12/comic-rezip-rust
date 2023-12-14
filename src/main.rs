use comic_rezip::helper;
// use chardet::detect;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use std::env;
use std::path::Path;
// use std::sync::Arc;
use chalk_rs::Chalk;
use std::fs::{self, File};
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

fn unzip(path: &str) {
    if let Ok(reader) = File::open(path) {
        if let Ok(mut zip) = ZipArchive::new(reader) {
            for i in 0..zip.len() {
                if let Ok(file) = zip.by_index(i) {
                    let file_name = file.name_raw();
                    let (encode, confidence, _) = chardet::detect(file_name);
                    if encode != "ascii" && encode != "utf-8" {
                        println!(
                            "encode: {}, confidence: {}, raw: {}",
                            encode,
                            confidence,
                            String::from_utf8_lossy(file_name)
                        );
                        if let Some(coder) =
                            encoding_from_whatwg_label(chardet::charset2encoding(&encode))
                        {
                            if let Ok(file_name) = coder.decode(&file_name, DecoderTrap::Ignore) {
                                if helper::validate_file_name(file_name.as_str()) {
                                    println!(
                                        "detect: {} {}",
                                        if file.is_dir() {
                                            Chalk::new().green().string(&"D").to_string()
                                        } else {
                                            Chalk::new().yellow().string(&"F").to_string()
                                        },
                                        file_name
                                    );
                                } else {
                                    println!("found illegal file name!: {}", file_name)
                                }
                            } else {
                                println!("解码错误!")
                            }
                        } else {
                            println!("get coder error")
                        }
                    }

                    // if let Some(coder) = encoding_from_whatwg_label(chardet::charset2encoding(
                    //     &String::from("GB18030"),
                    // )) {
                    //     if let Ok(utf8reader) = coder.decode(&file_name, DecoderTrap::Ignore) {
                    //         println!("GB18030: {}", utf8reader)
                    //     }
                    // }
                } else {
                    println!("zip.by_index {} failed!", i);
                }
            }
        }
    }
}

fn scan_dir(path: &str) {
    if let Ok(dir) = fs::read_dir(path) {
        for entry in dir {
            if let Ok(ok_entry) = entry {
                if let Some(full_path) = Path::new(path).join(ok_entry.file_name()).to_str() {
                    println!("path: {:?}", full_path);

                    if let Ok(file_type) = ok_entry.file_type() {
                        if file_type.is_dir() && !full_path.contains("node_modules") {
                            scan_dir(full_path);
                        } else if file_type.is_file() && full_path.ends_with(".zip") {
                            unzip(full_path);
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);
    scan_dir(&args[1]);

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
