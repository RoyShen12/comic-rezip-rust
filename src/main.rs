use chalk_rs::Chalk;
use comic_rezip::{helper, CustomError, MyError};
use std::fs::{self, File};
use std::io;
use std::path::Path;
use std::{collections::HashMap, env};
use walkdir::WalkDir;
use zip::ZipArchive;

const OUT_PATH: &str = "Downloads/test-out";

fn unzip(path: &str) -> Result<(HashMap<String, u32>, String, String), MyError> {
    let mut ret: HashMap<String, u32> = HashMap::new();

    let reader = File::open(path)?;
    let mut zip = ZipArchive::new(reader)?;

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
                fs::create_dir_all(out_path_parent.unwrap())?;
            }

            // create file fd
            println!("create file fd: {:?}", out_path);
            let mut out_file = File::create(&out_path)?;

            // unzip file
            io::copy(&mut file, &mut out_file)?;
        } else {
            // mkdir dir from zip
            println!("mkdir -p {:?}", out_path);
            fs::create_dir_all(out_path)?;
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
fn scan_dir(path: &str) {
    // println!("goto path: {:?}", path);
    for entry in WalkDir::new(path) {
        if let Some(ok_entry) = entry.as_ref().ok() {
            let path = Path::new(path).join(ok_entry.file_name());
            if let Some(full_path) = path.to_str() {
                let file_type = ok_entry.file_type();

                if file_type.is_file() && full_path.ends_with(".zip") {
                    match unzip(full_path) {
                        Ok((map, temp_path_str, dest_file)) => {
                            println!("unzip {} Result", Chalk::new().bold().string(&full_path));
                            for (k, v) in &map {
                                println!(
                                    "{k}->{}->{v}",
                                    mime_guess::from_ext(k.as_str())
                                        .first_or_octet_stream()
                                        .to_string()
                                )
                            }

                            match helper::zip_dir(&temp_path_str, &dest_file) {
                                Ok(()) => {}
                                Err(e) => eprintln!("{e}"),
                            }

                            // clean temp dir
                            match fs::remove_dir_all(&temp_path_str) {
                                Ok(()) => println!("clean tmp dir ok"),
                                Err(e) => eprintln!("{e}"),
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "unzip {} failed: {:?}",
                                Chalk::new().bold().string(&full_path),
                                e
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
        } else {
            eprintln!(
                "{}",
                Chalk::new()
                    .light_red()
                    .string(&format!("cannot walk into entry {:?}", entry))
            );
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    // println!("{args:?}");
    let _ = scan_dir(&args[1]);
}
