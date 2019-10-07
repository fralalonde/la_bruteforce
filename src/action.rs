use crate::schema::{Parameter, Mode};
use std::fmt;

/// possibly expands to multiple field keys
pub struct QueryKey {
    pub name: String,
    pub param: Parameter,
    pub index: Option<usize>,
    pub mode: Option<Mode>,
}

impl fmt::Display for QueryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if let Some(index) = &self.index {
            f.write_fmt(format_args!("/{}", index))?;
        }
        if let Some(mode) = &self.mode {
            f.write_fmt(format_args!(":{}", mode))?;
        }
        Ok(())
    }
}

// TODO FieldKey for receive
/// FieldKeys
/// Multiple FieldKeys fold back to QueryKey with Mode
pub struct FieldKey {
    pub name: String,
    pub param: Parameter,
    pub index: Option<usize>,
    pub field: Option<Field>
}

