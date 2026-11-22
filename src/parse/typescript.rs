use crate::error::PackError;

pub fn extract(source: &str) -> Result<super::rust::Extracted, PackError> {
    let tsx = false;
    super::javascript::extract_typescript(source, tsx)
}
