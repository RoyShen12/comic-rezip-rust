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
}
