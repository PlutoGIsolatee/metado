use std::collections::HashMap;
use std::io::Read;

pub struct Container {
    files: HashMap<String, Vec<u8>>,
}

impl Container {
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let reader = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("invalid ZIP: {}", e))?;

        let mut files = HashMap::new();
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| format!("ZIP read error: {}", e))?;
            if file.is_dir() {
                continue;
            }

            let name = file.name().to_string();
            let mut content = Vec::new();
            file.read_to_end(&mut content)
                .map_err(|e| format!("read error: {}", e))?;
            files.insert(name, content);
        }

        if !files.contains_key("mdl.toml") {
            return Err("missing mdl.toml in container".into());
        }

        Ok(Self { files })
    }

    pub fn file_exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        // Block path traversal
        if path.contains("..") || path.starts_with('/') {
            return Err(format!("path traversal blocked: {}", path));
        }
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("file not found: {}", path))
    }

    pub fn files(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.files.iter()
    }
}