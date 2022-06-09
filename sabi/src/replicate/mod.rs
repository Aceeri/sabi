use std::borrow::Cow;
use std::collections::hash_map::{DefaultHasher, Entry};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};

use bevy::prelude::*;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

pub mod collider;
pub mod general;
pub mod physics;

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
    pub replicate: ReplicateTypes,
}

impl Types {
    pub fn to_toml(&self) -> String {
        let replicate_toml = self.replicate.to_toml();
        format!("{}\n", replicate_toml)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicateTypes(HashMap<String, u16>);

impl ReplicateTypes {
    pub fn to_toml(&self) -> String {
        let types = self
            .0
            .iter()
            .map(|(key, value)| format!("\"{}\"={}\n", key, value))
            .collect::<String>();

        format!("[replicate]\n{}", types)
    }
}

lazy_static::lazy_static! {
    pub static ref TYPES: Arc<RwLock<Types>> = Arc::new(RwLock::new(read_types_file()));
}

pub fn read_types_file() -> Types {
    use std::io::Read;

    let mut file = std::fs::File::open("types.toml").expect("open types.toml");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("read types.toml");

    let types: Types = toml::from_str(&contents).expect("parse types.toml");
    types
}

pub fn write_types_file() {
    use std::io::Write;

    let types = TYPES.read().expect("read TYPES so we can write");
    let mut file = std::fs::File::create("types.toml").expect("open types.toml");
    let new_toml = types.to_toml();
    file.write_all(new_toml.as_bytes())
        .expect("write to types.toml");
    file.flush().expect("could not flush to types.toml");
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReplicateId(pub u16);

pub trait Replicate
where
    Self: Sized,
{
    type Def: Serialize + for<'de> Deserialize<'de>;
    fn into_def(self) -> Self::Def;
    fn from_def(def: Self::Def) -> Self;
    fn apply_def(&mut self, def: Self::Def) {
        *self = Self::from_def(def);
    }
    fn replicate_id() -> ReplicateId {
        let long_id = std::any::type_name::<Self>().to_owned();

        let read_lock = TYPES.read().expect("read TYPES");
        let short_id = match read_lock.replicate.0.get(&long_id) {
            Some(short_id) => *short_id,
            None => {
                drop(read_lock);

                info!("adding new type to types.toml: {}", long_id);
                let mut write_lock = TYPES.write().expect("could not write short id");
                let next_id = write_lock.replicate.0.values().max().cloned().unwrap_or(0) + 1;
                write_lock.replicate.0.insert(long_id, next_id);
                drop(write_lock);

                write_types_file();
                next_id
            }
        };

        ReplicateId(short_id)
    }
}

pub enum ReplicationMark<C>
where
    C: 'static + Component + Replicate,
{
    /// Make sure the component gets to the client once so it knows to have it on the entity.
    ///
    /// Past that never re-sends.
    Once(PhantomData<C>),
    /// Sends a component whenever it is highest in priority.
    Constant(PhantomData<C>),
}
