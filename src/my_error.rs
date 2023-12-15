use std::fmt;

use chalk_rs::Chalk;
use zip::result::ZipError;

pub struct CustomError {
    // 在这里添加你需要的字段
    pub message: String,
}

impl CustomError {
    pub fn new(msg: &str) -> CustomError {
        CustomError {
            message: String::from(msg),
        }
    }
}

// 实现 `std::fmt::Display` trait, which is used to display the error message.
impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            Chalk::new().light_red().string(&self.message.clone())
        )
    }
}

// 实现 `std::fmt::Debug` trait，为错误提供调试信息。
impl fmt::Debug for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CustomError {{ message: {} }}",
            Chalk::new().light_red().string(&self.message.clone())
        )
    }
}

// Finally, 实现 `std::error::Error` trait.
impl std::error::Error for CustomError {
    // This function should return a short description of the error.
    fn description(&self) -> &str {
        &self.message
    }

    // This function allows you to provide more detailed information, but it can also return None.
    fn cause(&self) -> Option<&dyn std::error::Error> {
        None
    }
}

#[derive(Debug)]
pub enum MyError {
    Io(std::io::Error),
    Parse(std::num::ParseIntError),
    Zip(ZipError),
    Custom(CustomError),
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MyError::Io(ref err) => write!(f, "IO error: {err}"),
            MyError::Parse(ref err) => write!(f, "Parse error: {err}"),
            MyError::Zip(ref err) => write!(f, "Zip Lib error: {err}"),
            MyError::Custom(ref err) => write!(f, "custom error: {err}",),
        }
    }
}

impl std::error::Error for MyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            MyError::Io(ref err) => Some(err),
            MyError::Parse(ref err) => Some(err),
            MyError::Zip(ref err) => Some(err),
            MyError::Custom(ref err) => Some(err),
        }
    }
}

impl From<std::io::Error> for MyError {
    fn from(err: std::io::Error) -> MyError {
        MyError::Io(err)
    }
}
impl From<std::num::ParseIntError> for MyError {
    fn from(err: std::num::ParseIntError) -> MyError {
        MyError::Parse(err)
    }
}
impl From<ZipError> for MyError {
    fn from(err: ZipError) -> MyError {
        MyError::Zip(err)
    }
}
impl From<CustomError> for MyError {
    fn from(err: CustomError) -> MyError {
        MyError::Custom(err)
    }
}
