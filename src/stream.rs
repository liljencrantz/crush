use std::collections::VecDeque;

pub struct Stream {
    row_type: Vec<crate::result::CellType>,
    closed: bool,
    data: VecDeque<create::result::Row>
}

impl Stream {
    fn next(&mut self) -> Option<crate::result::Row> {

    }

    fn add(&mut self, row: &crate::result::Row) {
        self.data.append(crate::result::Row::from(row))
    }

    fn close(&mut self) {
        self.closed = true;
    }
}
