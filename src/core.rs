use midi::MidiValue;
use std::cmp::Eq;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

use linked_hash_map::LinkedHashMap;

pub type Result<T> = ::std::result::Result<T, Box<::std::error::Error>>;

pub trait ParamEnum: Sized + Eq + Hash + 'static + fmt::Debug + Copy {
    /// Safely convert from midi value to enum by iterating through valid values for one that matches.
    /// Return `None` if none of the enum values have that integer id.
    /// `mem::transmute()` could be faster... but wrong.
    fn from_midi(int_value: u8) -> Option<Self>;

    /// Shortcut method to pick enum from its declaration position.
    /// Return `None` if none of the enum values have that integer id.
    fn from_ordinal(index: usize) -> Option<Self>;

    /// Get the ordinal value of the enum.
    /// First enum  defined is 0, second is 1...
    fn ordinal(&self) -> usize;

    /// Get the ordinal value of the enum.
    /// First enum  defined is 0, second is 1...
    fn enum_values() -> Vec<Self>;
}

/// Generate a rich C-style enum where
/// - each enum value has an associated integer value
/// - none of the enum values have fields
macro_rules! param_enum {
{ $id:ident { $($key:ident => $value:expr,)* } } => {
    #[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone)]
    pub enum $id {
       $($key = $value,)*
    }

    impl ::core::ParamEnum for $id {
        /// Safely convert from midi value to enum by iterating through valid values for one that matches.
        /// Return `None` if none of the enum values have that integer id.
        /// `mem::transmute()` could be faster... but wrong.
        fn from_midi(int_value: u8) -> Option<Self> {
            for ev in Self::enum_values() {
                if int_value == ev as u8 {
                    return Some(ev);
                }
            }
            None
        }

        /// Shortcut method to pick enum from its declaration position.
        /// Return `None` if none of the enum values have that integer id.
        #[inline]
        fn from_ordinal(index: usize) -> Option<Self> {
            Self::enum_values().get(index).cloned()
        }


        /// Get the ordinal value of the enum.
        /// First enum  defined is 0, second is 1...
        fn ordinal(&self) -> usize {
            for (idx, ev) in Self::enum_values().iter().enumerate() {
                if self == ev {
                    return idx;
                }
            }
            panic!("Enum has no ordinal value ?! {:?}", self);
        }

        fn enum_values() -> Vec<Self> {
            vec![
                $($key,)*
            ]
        }
    }
}}

pub trait ParameterDef {
    fn name(&self) -> &str;

    fn values(&self) -> Vec<MidiValue>;

    fn value_name(&self, value: MidiValue) -> Option<String> {
        Some(format!("{:x}", value))
    }
}

#[derive(Default, Debug)]
pub struct DiscreteParameter {
    name: String,
    shortcut: Option<char>,
    values: HashMap<MidiValue, String>,
}

pub fn discrete(
    name: &str,
    shortcut: Option<char>,
    values: Vec<(u8, &str)>,
) -> Box<ParameterDef + Send + std::marker::Sync> {
    let values = values
        .iter()
        .map(|(id, name)| (*id, name.to_string()))
        .collect();
    Box::new(DiscreteParameter {
        name: name.to_string(),
        shortcut,
        values,
    })
}

impl ParameterDef for DiscreteParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn values(&self) -> Vec<MidiValue> {
        self.values.keys().map(|k| *k).collect()
    }

    fn value_name(&self, key: MidiValue) -> Option<String> {
        self.values.get(&key).cloned()
    }
}

#[derive(Default, Debug)]
pub struct RangeParameter {
    name: String,
    shortcut: Option<char>,
    value_range: (MidiValue, MidiValue),
}

pub fn range(
    name: &str,
    shortcut: Option<char>,
    value_range: (u8, u8),
) -> Box<ParameterDef + Send + std::marker::Sync> {
    Box::new(RangeParameter {
        name: name.to_string(),
        shortcut,
        value_range,
    })
}

impl ParameterDef for RangeParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn values(&self) -> Vec<MidiValue> {
        (self.value_range.0..=self.value_range.1).collect()
    }
}

pub trait Device: fmt::Debug {
    type PARAM: Hash + Eq;

    fn parameters(
        &self,
    ) -> &'static LinkedHashMap<Self::PARAM, Box<ParameterDef + Send + std::marker::Sync>>;

    /// Get current parameter value if set.
    fn get(&self, p: Self::PARAM) -> Option<MidiValue>;

    /// Set new parameter value.
    /// Returns previous value if any.
    fn set(&mut self, p: Self::PARAM, value: MidiValue) -> Result<Option<MidiValue>>;
}
