use std::path::{Path, PathBuf};

pub struct ModuleLoader {
    root: PathBuf,
}

impl ModuleLoader {
    pub fn new(root: &str) -> Self {
        Self {
            root: PathBuf::from(root),
        }
    }

    pub fn resolve(&self, specifier: &str, from: &str) -> Result<PathBuf, String> {
        let from_dir = Path::new(from).parent().unwrap_or(&self.root);

        // Absolute
        if specifier.starts_with('/') {
            return Ok(PathBuf::from(specifier));
        }

        // Relative
        if specifier.starts_with('.') {
            let resolved = from_dir.join(specifier);
            return self.try_with_extension(&resolved);
        }

        // node_modules
        self.resolve_node_modules(specifier, from_dir)
    }

    fn try_with_extension(&self, path: &Path) -> Result<PathBuf, String> {
        if path.exists() {
            return Ok(path.to_path_buf());
        }
        let with_ext = path.with_extension("js");
        if with_ext.exists() {
            return Ok(with_ext);
        }
        Err(format!("module not found: {}", path.display()))
    }

    fn resolve_node_modules(&self, specifier: &str, from_dir: &Path) -> Result<PathBuf, String> {
        let mut current = from_dir.to_path_buf();
        loop {
            let candidate = current.join("node_modules").join(specifier);
            if candidate.exists() {
                return Ok(candidate);
            }
            let with_ext = candidate.with_extension("js");
            if with_ext.exists() {
                return Ok(with_ext);
            }
            if !current.pop() {
                break;
            }
        }
        Err(format!("module not found: {}", specifier))
    }
}