use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use crate::synth::sampler::{SampleBuffer, load_audio_file};

/// A shared bank of preloaded audio samples.
pub struct SampleBank {
    /// Maps relative sample path keys (e.g., "loops/amen.wav") to shared sample buffers.
    pub samples: HashMap<String, Arc<SampleBuffer>>,
}

impl SampleBank {
    /// Creates a new empty `SampleBank`.
    pub fn new() -> Self {
        Self {
            samples: HashMap::new(),
        }
    }

    /// Recursively scans a directory loading all supported `.wav`, `.mp3`, `.ogg`, `.aiff`, and `.flac` audio files.
    pub fn load_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<(), anyhow::Error> {
        let base_path = path.as_ref();
        if !base_path.exists() {
            return Err(anyhow::anyhow!("Directory does not exist: {:?}", base_path));
        }
        
        let root_name = base_path.file_name().and_then(|n| n.to_str()).unwrap_or("Samples");
        println!("{}", root_name);

        self.visit_dirs(base_path, base_path)?;
        Ok(())
    }

    /// Internal recursive directory visitor helper.
    fn visit_dirs(&mut self, dir: &Path, base_path: &Path) -> Result<(), anyhow::Error> {
        if dir.is_dir() {
            if dir != base_path {
                if let Ok(rel_dir) = dir.strip_prefix(base_path) {
                    let depth = rel_dir.components().count();
                    let dashes_count = 6 + (depth.saturating_sub(1)) * 10;
                    let dashes = "-".repeat(dashes_count);
                    let folder_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    println!("{} {}", dashes, folder_name);
                }
            }

            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    self.visit_dirs(&path, base_path)?;
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "wav" || ext_lower == "mp3" || ext_lower == "ogg" || ext_lower == "aiff" || ext_lower == "flac" {
                        if let Ok(rel_path) = path.strip_prefix(base_path) {
                            let key = rel_path.to_string_lossy().into_owned();
                            let depth = rel_path.components().count();
                            let dashes_count = 6 + (depth.saturating_sub(1)) * 10;
                            let dashes = "-".repeat(dashes_count);
                            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(&key);
                            println!("{} {}", dashes, file_name);
                            match load_audio_file(&path) {
                                Ok(buf) => {
                                    self.samples.insert(key, Arc::new(buf));
                                }
                                Err(e) => {
                                    eprintln!("{} [Error] {}: {:?}", dashes, file_name, e);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Looks up a sample by name, returning a reference-counted clone if found.
    pub fn get(&self, name: &str) -> Option<Arc<SampleBuffer>> {
        self.samples.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_bank_empty() {
        let bank = SampleBank::new();
        assert!(bank.get("nonexistent.wav").is_none());
    }
}
