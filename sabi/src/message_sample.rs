use std::{
    collections::hash_map::DefaultHasher,
    fs::File,
    hash::Hasher,
    io::{Read, Write},
    path::PathBuf,
};

pub fn try_add_sample<S: AsRef<str>>(kind: S, data: &[u8]) {
    if let Err(err) = add_sample(kind.as_ref(), data) {
        bevy::log::error!("add `{}` sample failed: {}", kind.as_ref(), err);
    }
}

/// Add a sample message to our library that we can use for a Zstd dictionary.
pub fn add_sample<S: AsRef<str>>(kind: S, data: &[u8]) -> Result<(), std::io::Error> {
    let dir_path = sample_dir_path(kind);
    let file_name = file_name(data);
    std::fs::create_dir_all(dir_path.clone())?;

    let file_path = {
        let mut file_path = dir_path.clone();
        file_path.push(file_name);
        file_path
    };

    let mut file = File::create(file_path)?;
    file.write_all(data)?;
    file.flush()?;
    Ok(())
}

pub fn create_dictionary<S: AsRef<str>>(kind: S) -> Result<Vec<u8>, std::io::Error> {
    let (samples, max_size) = samples(kind.as_ref())?;
    let dict = zstd::dict::from_files(&samples, max_size)?;
    Ok(dict)
}

pub fn read_dictionary<S: AsRef<str>>(kind: S) -> Result<Vec<u8>, std::io::Error> {
    let file_path = dict_file_path(kind.as_ref());

    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    Ok(buffer)
}

pub fn store_dictionary<S: AsRef<str>>(kind: S) -> Result<(), std::io::Error> {
    let dict = create_dictionary(kind.as_ref())?;
    let file_path = dict_file_path(kind.as_ref());

    let mut file = File::create(file_path)?;
    file.write_all(&dict)?;
    file.flush()?;
    Ok(())
}

pub fn samples<S: AsRef<str>>(kind: S) -> Result<(Vec<PathBuf>, usize), std::io::Error> {
    let dir_path = sample_dir_path(kind);
    let mut files = Vec::new();

    let mut max_size = 0;
    for entry in std::fs::read_dir(dir_path)? {
        let entry = entry?;

        let metadata = entry.metadata()?;
        let len = metadata.len() as usize;
        if len > max_size {
            max_size = len;
        }

        if let Some("sample") = entry.path().extension().and_then(|ext| ext.to_str()) {
            files.push(entry.path().canonicalize()?);
        }
    }

    Ok((files, max_size))
}

pub fn dict_file_path<S: AsRef<str>>(kind: S) -> PathBuf {
    let dir_path = sample_dir_path(kind.as_ref());

    let file_path = {
        let mut file_path = dir_path.clone();
        file_path.push(format!("{}.dict", kind.as_ref()));
        file_path
    };

    file_path
}

pub fn dict_path<S: AsRef<str>>(kind: S) -> PathBuf {
    PathBuf::from(format!("./dictionary/{}.dict", kind.as_ref()))
}

pub fn sample_dir_path<S: AsRef<str>>(kind: S) -> PathBuf {
    PathBuf::from(format!("./messages/{}/", kind.as_ref()))
}

pub fn file_name(data: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();

    for byte in data {
        hasher.write_u8(*byte);
    }

    format!("{}.sample", hasher.finish())
}
