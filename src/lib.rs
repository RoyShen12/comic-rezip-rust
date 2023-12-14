pub mod helper {
    pub fn validate_file_name(file_name: &str) -> bool {
        if file_name.contains('\\') {
            return false;
        }
        if (file_name.len() >= 2
            && file_name.chars().nth(1) == Some(':')
            && file_name.chars().nth(0).unwrap().is_alphabetic())
            || file_name.starts_with('/')
        {
            return false;
        }
        if file_name.split('/').any(|part| part == "..") {
            return false;
        }

        // all good
        return true;
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
}
