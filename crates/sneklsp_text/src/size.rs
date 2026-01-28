use std::ops::{Add, Sub};

/// byte offset in source txt
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextSize(u32);

impl TextSize {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn of(text: &str) -> Self {
        Self(text.len() as u32)
    }

    pub const fn to_u32(self) -> u32 {
        self.0
    }

    pub const fn to_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for TextSize {
    fn from(raw: u32) -> Self {
        Self(raw)
    }
}

impl From<usize> for TextSize {
    fn from(raw: usize) -> Self {
        Self(raw as u32)
    }
}

impl Add for TextSize {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for TextSize {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
