use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, RwLock};

use bevy::prelude::*;
use bevy::reflect::FromReflect;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

//pub mod general;
//pub mod physics2d;
pub mod physics3d;

pub fn deserialize_number_from_string<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr + serde::Deserialize<'de>,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt<T> {
        String(String),
        Number(T),
    }

    match StringOrInt::<T>::deserialize(deserializer)? {
        StringOrInt::String(s) => s.parse::<T>().map_err(serde::de::Error::custom),
        StringOrInt::Number(i) => Ok(i),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Types {
    #[serde(default)]
    pub replicate: ReplicateTypes,
}

impl Types {
    pub fn to_toml(&self) -> String {
        let replicate_toml = self.replicate.to_toml();
        format!("{}\n", replicate_toml)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ReplicateTypes(HashMap<String, u16>);

impl ReplicateTypes {
    // Do the serialization ourselves because of bugs with the `toml` crate.
    pub fn to_toml(&self) -> String {
        let types = self
            .0
            .iter()
            .map(|(key, value)| format!("\"{}\" = {}\n", key, value))
            .collect::<String>();

        format!("[replicate]\n{}", types)
    }

    pub fn next_id(&self) -> u16 {
        self.0.iter().map(|(_name, ty)| ty).max().unwrap_or(&0) + 1
    }

    pub fn from_id(&self, id: u16) -> Option<String> {
        self.0
            .iter()
            .find(|(_, replicate_id)| **replicate_id == id)
            .map(|(name, _)| name.clone())
    }
}

lazy_static::lazy_static! {
    pub static ref TYPES: Arc<RwLock<Types>> = Arc::new(RwLock::new(read_types_file()));
}

pub const TYPES_PATH: &'static str = "types.toml";

pub fn read_types_file() -> Types {
    use std::io::Read;

    let mut file = match std::fs::File::open(TYPES_PATH) {
        Ok(file) => file,
        Err(err) => std::fs::File::create(TYPES_PATH).expect("could not create types.toml"),
    };
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("read types.toml");

    let types: Types = toml::from_str(&contents).expect("parse types.toml");
    types
}

pub fn write_types_file() {
    use std::io::Write;

    let types = TYPES.read().expect("read TYPES so we can write");
    let mut file = std::fs::File::create(TYPES_PATH).expect("open types.toml");
    let new_toml = types.to_toml();
    file.write_all(new_toml.as_bytes())
        .expect("write to types.toml");
    file.flush().expect("could not flush to types.toml");
}

/// Smaller unique id per type for serialization so it is easier to compress for network packets.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReplicateId(pub u16);

impl ReplicateId {
    pub fn name(&self) -> String {
        let types = TYPES.read().expect("read TYPES so we can write");
        types.replicate.from_id(self.0).unwrap()
    }
}

/// An id that should be the same over time/builds/etc. so that the server and client can
/// accurately communicate with eachother.
///
/// Currently this is persistent based on the `types.toml` file in the project folder.
/// If this file is cleared then it may not be the same in the next build.
pub fn replicate_id<T>() -> ReplicateId
where
    T: 'static + Reflect + FromReflect,
{
    let long_id = std::any::type_name::<T>().to_owned();

    let read_lock = TYPES.read().expect("read TYPES");
    let short_id = match read_lock.replicate.0.get(&long_id) {
        Some(short_id) => *short_id,
        None => {
            drop(read_lock);

            info!("adding new type to types.toml: {}", long_id);
            let mut write_lock = TYPES.write().expect("could not write short id");
            let next_id = write_lock.replicate.next_id();
            write_lock.replicate.0.insert(long_id, next_id);
            drop(write_lock);

            write_types_file();
            next_id
        }
    };

    ReplicateId(short_id)
}
