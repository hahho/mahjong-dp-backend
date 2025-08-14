use std::{
    fs::{create_dir_all, rename, File},
    io::{BufReader, BufWriter},
    path::Path,
};

use anyhow::Result;

use serde;

pub fn save_object<T: serde::Serialize, U: AsRef<Path>>(filename: U, content: &T) -> Result<()> {
    let tempname = filename.as_ref().to_str().unwrap().to_string() + ".temp";
    create_dir_all(Path::new(&tempname).parent().unwrap())?;
    bincode::serialize_into(BufWriter::new(File::create(&tempname)?), content)?;
    rename(tempname, filename)?;
    Ok(())
}

pub fn load_object<T: serde::de::DeserializeOwned, U: AsRef<Path>>(filename: U) -> Result<T> {
    bincode::deserialize_from(BufReader::new(File::open(filename)?))
        .map_err(|e| anyhow::Error::new(e))
}
