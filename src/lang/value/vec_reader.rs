use chrono::Duration;
use crate::CrushResult;
use crate::data::table::{ColumnType, Row};
use crate::lang::errors::eof_error;
use crate::lang::pipe::CrushStream;
use crate::lang::value::{Value, ValueType};
use crate::util::replace::Replace;

pub struct VecReader {
    vec: Vec<Value>,
    types: Vec<ColumnType>,
    idx: usize,
}

impl VecReader {
    pub fn new(
        vec: Vec<Value>,
        column_type: ValueType,
    ) -> VecReader {
        VecReader {
            vec,
            types: vec![ColumnType::new("value", column_type)],
            idx: 0,
        }
    }
}

impl CrushStream for VecReader {
    fn read(&mut self) -> CrushResult<Row> {
        self.idx += 1;
        if self.idx > self.vec.len() {
            return eof_error()
        }
        Ok(Row::new(vec![self.vec.replace(self.idx - 1, Value::Empty)]))
    }

    fn read_timeout(
        &mut self,
        _timeout: Duration,
    ) -> Result<Row, crate::lang::pipe::RecvTimeoutError> {
        match self.read() {
            Ok(r) => Ok(r),
            Err(_) => Err(crate::lang::pipe::RecvTimeoutError::Disconnected),
        }
    }

    fn types(&self) -> &[ColumnType] {
        &self.types
    }
}
