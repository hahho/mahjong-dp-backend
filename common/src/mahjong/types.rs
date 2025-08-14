use std::io::{Read, Write};
use anyhow::Result;
use crate::flat_file_vec::FixedRepr;

pub const NUM_ROUNDS: usize = 18;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tile {
    Supai(u8, u8),
    Jihai(u8),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Dimension {
    Shuntsu(Tile), // tile denote the lowest one
    Kotsu(Tile),
    Toitsu(Tile),
    Kokushi,
}

const ID_TO_DIMENSION: [Dimension; Dimension::len()] = {
    let mut memo = [Dimension::Kokushi; Dimension::len()];
    let mut j = 0;
    while j < Dimension::len() {
        let mut i = j as i8;
        if i < 21 {
            memo[j] = Dimension::Shuntsu(Tile::Supai((i / 7) as u8, (i % 7) as u8));
            j += 1;
            continue
        }
        i -= 21;
        if i < 27 {
            memo[j] = Dimension::Kotsu(Tile::Supai((i / 9) as u8, (i % 9) as u8));
            j += 1;
            continue
        }
        i -= 27;
        if i < 5 {
            memo[j] = Dimension::Kotsu(Tile::Jihai(i as u8));
            j += 1;
            continue
        }
        i -= 5;
        if i < 27 {
            memo[j] = Dimension::Toitsu(Tile::Supai((i / 9) as u8, (i % 9) as u8));
            j += 1;
            continue
        }
        i -= 27;
        if i < 5{
            memo[j] = Dimension::Toitsu(Tile::Jihai(i as u8));
            j += 1;
            continue
        }
        j += 1;
    }
    memo
};

impl Dimension {
    pub fn from_id<T: Into<usize>>(id: T) -> Dimension {
        ID_TO_DIMENSION[id.into()]
    }
    
    pub fn to_id(&self) -> u8 {
        match self {
            Dimension::Shuntsu(Tile::Supai(x, y)) => x * 7 + y,
            Dimension::Kotsu(Tile::Supai(x, y)) => 21 + x * 9 + y,
            Dimension::Kotsu(Tile::Jihai(x)) => 21 + 27 + x,
            Dimension::Toitsu(Tile::Supai(x, y)) => 21 + 27 + 5 + x * 9 + y,
            Dimension::Toitsu(Tile::Jihai(x)) => 21 + 27 + 5 + 27 + x,
            Dimension::Kokushi => 85,
            _ => panic!("Invalid dimension: {:?}", self),
        }
    }
    
    pub const fn len() -> usize {
        86
    }

    /// Returns all dimension variants in ID order
    pub const fn all_dimensions() -> [Dimension; Self::len()] {
        ID_TO_DIMENSION
    }
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metrics {
    // Using a single array for all 86 dimensions, indexed by Dimension::to_id()
    pub values: [u32; Dimension::len()],
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    /// Create a new Metrics with all values set to 0
    pub fn new() -> Self {
        Self {
            values: [0; Dimension::len()],
        }
    }
}

// Convenience implementations for array-like access
impl std::ops::Index<Dimension> for Metrics {
    type Output = u32;
    
    fn index(&self, dim: Dimension) -> &Self::Output {
        &self.values[dim.to_id() as usize]
    }
}

impl std::ops::IndexMut<Dimension> for Metrics {
    fn index_mut(&mut self, dim: Dimension) -> &mut Self::Output {
        &mut self.values[dim.to_id() as usize]
    }
}

impl std::ops::Index<usize> for Metrics {
    type Output = u32;
    
    fn index(&self, id: usize) -> &Self::Output {
        &self.values[id]
    }
}

impl std::ops::IndexMut<usize> for Metrics {
    fn index_mut(&mut self, id: usize) -> &mut Self::Output {
        &mut self.values[id]
    }
}

// Conversion traits
impl From<[u32; Dimension::len()]> for Metrics {
    fn from(values: [u32; Dimension::len()]) -> Self {
        Self { values }
    }
}

impl From<Metrics> for [u32; Dimension::len()] {
    fn from(metrics: Metrics) -> Self {
        metrics.values
    }
}

impl AsRef<[u32]> for Metrics {
    fn as_ref(&self) -> &[u32] {
        &self.values
    }
}

impl AsMut<[u32]> for Metrics {
    fn as_mut(&mut self) -> &mut [u32] {
        &mut self.values
    }
}

impl FixedRepr for Metrics {
    const BYTE_SIZE: usize = Dimension::len() * 4;
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        for v in self.values.iter() {
            writer.write_all(&v.to_le_bytes())?;
        }
        Ok(())
    }

    fn deserialize<R: Read>(reader: &mut R) -> Result<Self> {
        let mut values = [0; Dimension::len()];
        for v in values.iter_mut() {
            *v = u32::deserialize(reader)?;
        }
        Ok(Self { values })
    }
}