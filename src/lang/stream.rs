use crate::stream::InputStream;
use crate::errors::{CrushResult, error};
use crate::lang::Row;

#[derive(Debug, Clone)]
pub struct TableStream {
    pub stream: InputStream,
}

impl TableStream {
    pub fn get(&self, idx: i128) -> CrushResult<Row> {
        let mut i = 0i128;
        loop {
            match self.stream.recv() {
                Ok(row) => {
                    if i == idx {
                        return Ok(row);
                    }
                    i += 1;
                },
                Err(_) => return error("Index out of bounds"),
            }
        }
    }

    pub fn reader(self) -> InputStream {
        self.stream
    }
}
