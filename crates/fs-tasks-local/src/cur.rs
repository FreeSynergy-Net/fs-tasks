//! A cursor over one row: read the columns in the order of the `SELECT`, no index to keep in step by hand.
//!
//! Pattern: Iterator-like reader (each call takes the next column; a value that cannot be read back is `Unavailable`).

use fs_objects::v5::codec::{
    Row, art_of, get_bool, get_id, get_int, get_opt_int, get_opt_text, get_text, reference_of,
};
use fs_tasks_testdata::tasks_v1::{ObjectId, PersonRef, TaskResult};
use turso::Value;

use crate::error::adapt;

/// The reader.
pub struct Cur<'a> {
    row: &'a Row,
    at: usize,
}

impl<'a> Cur<'a> {
    /// Start at the first column.
    #[must_use]
    pub fn new(row: &'a Row) -> Self {
        Self { row, at: 0 }
    }

    fn next(&mut self) -> usize {
        self.at += 1;
        self.at - 1
    }

    /// A required id.
    pub fn id(&mut self) -> TaskResult<ObjectId> {
        get_id(self.row, self.next()).map_err(adapt)
    }

    /// An id that may be empty.
    pub fn opt_id(&mut self) -> TaskResult<Option<ObjectId>> {
        let i = self.next();
        match self.row.get(i) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => get_id(self.row, i).map(Some).map_err(adapt),
        }
    }

    /// Required text.
    pub fn text(&mut self) -> TaskResult<String> {
        get_text(self.row, self.next()).map_err(adapt)
    }

    /// Text that may be empty.
    pub fn opt_text(&mut self) -> TaskResult<Option<String>> {
        get_opt_text(self.row, self.next()).map_err(adapt)
    }

    /// A required whole number.
    pub fn int(&mut self) -> TaskResult<i64> {
        get_int(self.row, self.next()).map_err(adapt)
    }

    /// A whole number that may be empty.
    pub fn opt_int(&mut self) -> TaskResult<Option<i64>> {
        get_opt_int(self.row, self.next()).map_err(adapt)
    }

    /// A required flag.
    pub fn flag(&mut self) -> TaskResult<bool> {
        get_bool(self.row, self.next()).map_err(adapt)
    }

    /// A person (three columns: id, kind, excerpt).
    pub fn person(&mut self) -> TaskResult<PersonRef> {
        let id = reference_of(&self.text()?).map_err(adapt)?;
        let art = art_of(&self.text()?).map_err(adapt)?;
        let abzug = self.opt_id()?;
        Ok(PersonRef { id, art, abzug })
    }

    /// A person that may be empty (three columns: all empty or the id and the kind set).
    pub fn opt_person(&mut self) -> TaskResult<Option<PersonRef>> {
        let (id, art, abzug) = (self.opt_text()?, self.opt_text()?, self.opt_id()?);
        match (id, art) {
            (Some(id), Some(art)) => Ok(Some(PersonRef {
                id: reference_of(&id).map_err(adapt)?,
                art: art_of(&art).map_err(adapt)?,
                abzug,
            })),
            _ => Ok(None),
        }
    }
}
