use std::{
    fs::{create_dir_all, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    marker::PhantomData,
    path::Path,
};

use anyhow::Result;

// A trait for types that can be serialized and deserialized from a fixed-size byte array.
pub trait FixedRepr: Default + Clone {
    const BYTE_SIZE: usize;
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()>;
    fn deserialize<R: Read>(reader: &mut R) -> Result<Self>;
}

// FixedRepr implementations for integer types
impl FixedRepr for u16 {
    const BYTE_SIZE: usize = 2;

    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }

    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        Ok(Self::from_le_bytes(buf))
    }
}

impl FixedRepr for u32 {
    const BYTE_SIZE: usize = 4;

    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }

    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        Ok(Self::from_le_bytes(buf))
    }
}

impl FixedRepr for u64 {
    const BYTE_SIZE: usize = 8;

    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }

    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;
        Ok(Self::from_le_bytes(buf))
    }
}

impl FixedRepr for u128 {
    const BYTE_SIZE: usize = 16;

    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }

    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 16];
        reader.read_exact(&mut buf)?;
        Ok(Self::from_le_bytes(buf))
    }
}

/// A flat file vector that stores elements in a file. It's just like a Vec, but the elements are stored in a file.
/// 
/// This structure provides methods to create, open, and manipulate a vector of elements stored in a file.
/// It supports basic operations like appending, extending, and clearing elements.
/// 
/// The file is opened in read-write mode by default, but can be opened in read-only mode if needed.
/// The file is automatically created if it doesn't exist.
#[derive(Debug)]
pub struct FlatFileVec<T: FixedRepr> {
    file: File,
    len: usize,
    _phantom: PhantomData<T>,
}

impl<T: FixedRepr> FlatFileVec<T> {
    /// Create a new empty flat file vector
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        Ok(Self {
            file,
            len: 0,
            _phantom: PhantomData,
        })
    }

    /// Open an existing flat file vector
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let file_size = file.metadata()?.len() as usize;

        if file_size % T::BYTE_SIZE != 0 {
            return Err(anyhow::Error::msg(
                "File size is not a multiple of element size",
            ));
        }

        let len = file_size / T::BYTE_SIZE;
        Ok(Self {
            file,
            len,
            _phantom: PhantomData,
        })
    }

    /// Open an existing flat file vector in read-only mode
    pub fn open_readonly<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let file_size = file.metadata()?.len() as usize;

        if file_size % T::BYTE_SIZE != 0 {
            return Err(anyhow::Error::msg(
                "File size is not a multiple of element size",
            ));
        }

        let len = file_size / T::BYTE_SIZE;
        Ok(Self {
            file,
            len,
            _phantom: PhantomData,
        })
    }

    /// Create a flat file vector from an existing File object
    pub fn from_file(file: File) -> Result<Self> {
        let file_size = file.metadata()?.len() as usize;

        if file_size % T::BYTE_SIZE != 0 {
            return Err(anyhow::Error::msg(
                "File size is not a multiple of element size",
            ));
        }

        let len = file_size / T::BYTE_SIZE;
        Ok(Self {
            file,
            len,
            _phantom: PhantomData,
        })
    }

    /// Open existing file or create new one if it doesn't exist
    pub fn open_or_create<P: AsRef<Path>>(path: P) -> Result<Self> {
        if path.as_ref().exists() {
            Self::open(path)
        } else {
            Self::create(path)
        }
    }

    /// Load all elements from a file path
    pub fn load_all<P: AsRef<Path>>(path: P) -> Result<Vec<T>> {
        let mut ffv = Self::open_readonly(path)?;
        let result = ffv.get_range(0, ffv.len())?;
        Ok(result)
    }

    /// Load all elements from a file
    pub fn load_all_from_file(file: File) -> Result<Vec<T>> {
        let mut ffv = Self::from_file(file)?;
        let result = ffv.get_range(0, ffv.len())?;
        Ok(result)
    }

    /// Save all elements to a file path
    pub fn save_all<P: AsRef<Path>, I>(items: I, path: P) -> Result<()>
    where
        I: IntoIterator<Item = T>,
    {
        let mut ffv = Self::open_or_create(path)?;
        ffv.extend(items)?;
        Ok(())
    }

    /// Save all elements to a file
    pub fn save_all_to_file<I>(items: I, file: File) -> Result<()>
    where
        I: IntoIterator<Item = T>,
    {
        let mut ffv = Self::from_file(file)?;
        ffv.extend(items)?;
        Ok(())
    }

    pub fn set_len(&mut self, len: usize) -> Result<()> {
        self.len = len;
        self.file.set_len(len as u64 * T::BYTE_SIZE as u64)?;
        Ok(())
    }

    /// Get the number of elements in the vector
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the vector is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a single element at the specified index
    pub fn get(&mut self, index: usize) -> Result<T> {
        if index >= self.len {
            return Err(anyhow::Error::msg("Index out of bounds"));
        }

        self.file
            .seek(SeekFrom::Start((index * T::BYTE_SIZE) as u64))?;
        T::deserialize(&mut self.file)
    }

    /// Get a range of elements [start, end)
    pub fn get_range(&mut self, start: usize, end: usize) -> Result<Vec<T>> {
        if start > end || end > self.len {
            return Err(anyhow::Error::msg("Invalid range"));
        }

        let count = end - start;
        let mut result = Vec::with_capacity(count);

        self.file
            .seek(SeekFrom::Start((start * T::BYTE_SIZE) as u64))?;
        let mut reader = BufReader::new(&mut self.file);

        for _ in 0..count {
            let element = T::deserialize(&mut reader)?;
            result.push(element);
        }

        Ok(result)
    }

    /// Append a single element to the end of the vector
    pub fn push(&mut self, item: &T) -> Result<()> {
        self.file.seek(SeekFrom::End(0))?;
        let mut writer = BufWriter::new(&mut self.file);
        item.serialize(&mut writer)?;
        writer.flush()?;
        self.len += 1;
        Ok(())
    }

    /// Append multiple elements to the end of the vector
    pub fn extend<I>(&mut self, items: I) -> Result<()>
    where
        I: IntoIterator<Item = T>,
    {
        let iter = items.into_iter();
        self.file.seek(SeekFrom::End(0))?;
        let mut writer = BufWriter::new(&mut self.file);

        let mut count = 0;
        for item in iter {
            item.serialize(&mut writer)?;
            count += 1;
        }
        
        writer.flush()?;
        self.len += count;
        Ok(())
    }

    /// Clear all elements from the vector
    pub fn clear(&mut self) -> Result<()> {
        // Truncate file to 0 bytes
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.len = 0;
        Ok(())
    }

    /// Get the current file position (useful for debugging)
    pub fn file_position(&mut self) -> Result<u64> {
        Ok(self.file.stream_position()?)
    }

    /// Sync all data to disk
    pub fn sync_all(&mut self) -> Result<()> {
        self.file.sync_all()?;
        Ok(())
    }

    /// Set a single element at the specified index
    pub fn set(&mut self, index: usize, value: &T) -> Result<()> {
        if index >= self.len {
            return Err(anyhow::Error::msg("Index out of bounds"));
        }

        self.file
            .seek(SeekFrom::Start((index * T::BYTE_SIZE) as u64))?;
        let mut writer = BufWriter::new(&mut self.file);
        value.serialize(&mut writer)?;
        writer.flush()?;
        Ok(())
    }

    /// Set a range of elements [start, start+values.len())
    pub fn set_range(&mut self, start: usize, values: &[T]) -> Result<()> {
        if start + values.len() > self.len {
            return Err(anyhow::Error::msg("Range out of bounds"));
        }

        if values.is_empty() {
            return Ok(());
        }

        self.file
            .seek(SeekFrom::Start((start * T::BYTE_SIZE) as u64))?;
        let mut writer = BufWriter::new(&mut self.file);

        for value in values {
            value.serialize(&mut writer)?;
        }
        writer.flush()?;
        Ok(())
    }

    /// Create an iterator over all elements in the vector
    pub fn iter(&mut self) -> Result<FlatFileVecIterator<T>> {
        self.file.seek(SeekFrom::Start(0))?;
        Ok(FlatFileVecIterator::new(self))
    }

    /// Create an iterator over a range of elements [start, end)
    pub fn iter_range(&mut self, start: usize, end: usize) -> Result<FlatFileVecIterator<T>> {
        if start > end || end > self.len {
            return Err(anyhow::Error::msg("Invalid range"));
        }
        self.file.seek(SeekFrom::Start((start * T::BYTE_SIZE) as u64))?;
        Ok(FlatFileVecIterator::new_with_range(self, start, end))
    }
}

/// Iterator for FlatFileVec that uses BufReader for efficient reading
pub struct FlatFileVecIterator<'a, T: FixedRepr> {
    reader: BufReader<&'a mut File>,
    current_index: usize,
    end_index: usize,
    _phantom: PhantomData<T>,
}

impl<'a, T: FixedRepr> FlatFileVecIterator<'a, T> {
    fn new(ffv: &'a mut FlatFileVec<T>) -> Self {
        let reader = BufReader::new(&mut ffv.file);
        
        Self {
            reader,
            current_index: 0,
            end_index: ffv.len,
            _phantom: PhantomData,
        }
    }

    fn new_with_range(ffv: &'a mut FlatFileVec<T>, start: usize, end: usize) -> Self {
        let reader = BufReader::new(&mut ffv.file);
        
        Self {
            reader,
            current_index: start,
            end_index: end,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: FixedRepr> Iterator for FlatFileVecIterator<'a, T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.end_index {
            return None;
        }

        let result = T::deserialize(&mut self.reader);
        self.current_index += 1;
        
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end_index - self.current_index;
        (remaining, Some(remaining))
    }
}

impl<'a, T: FixedRepr> ExactSizeIterator for FlatFileVecIterator<'a, T> {
    fn len(&self) -> usize {
        self.end_index - self.current_index
    }
}

// IntoIterator implementations for FlatFileVec
impl<T: FixedRepr> IntoIterator for FlatFileVec<T> {
    type Item = Result<T>;
    type IntoIter = FlatFileVecIntoIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        FlatFileVecIntoIterator::new(self)
    }
}

impl<'a, T: FixedRepr> IntoIterator for &'a mut FlatFileVec<T> {
    type Item = Result<T>;
    type IntoIter = FlatFileVecIterator<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        // This will panic if seek fails, but that's probably the desired behavior
        self.iter().expect("Failed to create iterator")
    }
}

/// Owned iterator for FlatFileVec
pub struct FlatFileVecIntoIterator<T: FixedRepr> {
    reader: BufReader<File>,
    current_index: usize,
    end_index: usize,
    _phantom: PhantomData<T>,
}

impl<T: FixedRepr> FlatFileVecIntoIterator<T> {
    fn new(mut ffv: FlatFileVec<T>) -> Self {
        // Seek to the beginning of the file
        let _ = ffv.file.seek(SeekFrom::Start(0));
        let reader = BufReader::new(ffv.file);
        
        Self {
            reader,
            current_index: 0,
            end_index: ffv.len,
            _phantom: PhantomData,
        }
    }
}

impl<T: FixedRepr> Iterator for FlatFileVecIntoIterator<T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.end_index {
            return None;
        }

        let result = T::deserialize(&mut self.reader);
        self.current_index += 1;
        
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end_index - self.current_index;
        (remaining, Some(remaining))
    }
}

impl<T: FixedRepr> ExactSizeIterator for FlatFileVecIntoIterator<T> {
    fn len(&self) -> usize {
        self.end_index - self.current_index
    }
}
