//! The `GeneralizedInput` is an input that ca be generalized to represent a rule, used by Grimoire

use ahash::AHasher;
use alloc::{borrow::ToOwned, rc::Rc, string::String, vec::Vec};
use core::hash::Hasher;
use core::{cell::RefCell, convert::From};
use serde::{Deserialize, Serialize};

#[cfg(feature = "std")]
use crate::Error;
#[cfg(feature = "std")]
use std::{fs::File, io::Read, path::Path};

use crate::{
    bolts::{ownedref::OwnedSlice, HasLen},
    inputs::{HasBytesVec, HasTargetBytes, Input},
};

/// An item of the generalized input
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GeneralizedItem {
    /// Real bytes
    Bytes(Vec<u8>),
    /// An insertion point
    Gap,
}

/// A bytes input with a generalized version mainly used for Grimoire
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GeneralizedInput {
    /// The raw input bytes
    bytes: Vec<u8>,
    generalized: Option<Vec<GeneralizedItem>>,
    /// If was mutated or not by Grimoire
    pub grimoire_mutated: bool,
}

impl Input for GeneralizedInput {
    /// Generate a name for this input
    fn generate_name(&self, _idx: usize) -> String {
        let mut hasher = AHasher::new_with_keys(0, 0);
        // TODO add generalized
        hasher.write(self.bytes());
        format!("{:016x}", hasher.finish())
    }

    /// An hook executed before being added to the corpus
    fn wrapped_as_testcase(&mut self) {
        // remove generalized for inputs generated with bit-level mutations
        if !self.grimoire_mutated {
            self.generalized = None;
        }
        // restore to allow bit-level mutations
        self.grimoire_mutated = false;
    }
}

/// Rc Ref-cell from Input
impl From<GeneralizedInput> for Rc<RefCell<GeneralizedInput>> {
    fn from(input: GeneralizedInput) -> Self {
        Rc::new(RefCell::new(input))
    }
}

impl HasBytesVec for GeneralizedInput {
    #[inline]
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[inline]
    fn bytes_mut(&mut self) -> &mut Vec<u8> {
        &mut self.bytes
    }
}

impl HasTargetBytes for GeneralizedInput {
    #[inline]
    fn target_bytes(&self) -> OwnedSlice<u8> {
        if self.grimoire_mutated {
            OwnedSlice::from(self.generalized_to_bytes())
        } else {
            OwnedSlice::from(&self.bytes)
        }
    }
}

impl HasLen for GeneralizedInput {
    #[inline]
    fn len(&self) -> usize {
        self.bytes.len()
    }
}

impl From<Vec<u8>> for GeneralizedInput {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

impl From<&[u8]> for GeneralizedInput {
    fn from(bytes: &[u8]) -> Self {
        Self::new(bytes.to_owned())
    }
}

impl GeneralizedInput {
    /// Creates a new bytes input using the given bytes
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            generalized: None,
            grimoire_mutated: false,
        }
    }

    /// Fill the generalized vector from a slice of option (None -> Gap)
    pub fn generalized_from_options(&mut self, v: &[Option<u8>]) {
        let mut res = vec![];
        let mut bytes = vec![];
        if v.first() != Some(&None) {
            res.push(GeneralizedItem::Gap);
        }
        for e in v {
            match e {
                None => {
                    if !bytes.is_empty() {
                        res.push(GeneralizedItem::Bytes(bytes.clone()));
                        bytes.clear();
                    }
                    res.push(GeneralizedItem::Gap);
                }
                Some(b) => {
                    bytes.push(*b);
                }
            }
        }
        if !bytes.is_empty() {
            res.push(GeneralizedItem::Bytes(bytes));
        }
        if res.last() != Some(&GeneralizedItem::Gap) {
            res.push(GeneralizedItem::Gap);
        }
        self.generalized = Some(res);
    }

    /// Extend the generalized input
    pub fn generalized_extend(&mut self, other: &[GeneralizedItem]) {
        let gen = self.generalized.get_or_insert_with(Vec::new);
        if gen.last().is_some()
            && other.first().is_some()
            && *gen.last().unwrap() == GeneralizedItem::Gap
            && *other.first().unwrap() == GeneralizedItem::Gap
        {
            gen.extend_from_slice(&other[1..]);
        } else {
            gen.extend_from_slice(other);
        }
    }

    /// Get the size of the generalized
    #[must_use]
    pub fn generalized_len(&self) -> usize {
        match &self.generalized {
            None => 0,
            Some(gen) => {
                let mut size = 0;
                for item in gen {
                    match item {
                        GeneralizedItem::Bytes(b) => size += b.len(),
                        GeneralizedItem::Gap => size += 1,
                    }
                }
                size
            }
        }
    }

    /// Convert generalized to bytes
    #[must_use]
    pub fn generalized_to_bytes(&self) -> Vec<u8> {
        match &self.generalized {
            None => vec![],
            Some(gen) => {
                let mut bytes = vec![];
                for item in gen {
                    if let GeneralizedItem::Bytes(b) = item {
                        bytes.extend_from_slice(b);
                    }
                }
                bytes
            }
        }
    }

    /// Get the generalized input
    #[must_use]
    pub fn generalized(&self) -> Option<&[GeneralizedItem]> {
        self.generalized.as_deref()
    }

    /// Get the generalized input (mut)
    pub fn generalized_mut(&mut self) -> &mut Option<Vec<GeneralizedItem>> {
        &mut self.generalized
    }

    /// Load from a plain file of bytes
    #[cfg(feature = "std")]
    pub fn from_bytes_file<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let mut file = File::open(path)?;
        let mut bytes: Vec<u8> = vec![];
        file.read_to_end(&mut bytes)?;
        Ok(Self {
            bytes,
            generalized: None,
            grimoire_mutated: false,
        })
    }
}
