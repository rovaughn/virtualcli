use std::io::Write;

// TODO: Probably better to replace the pub fields with a public function
//       interface.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct State {
    need_clear: bool,
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Vec<char>>,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

fn repeat_vec<T>(value: T, n: usize) -> Vec<T>
where T: Clone {
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(value.clone());
    }
    v
}

impl State {
    pub fn write(&mut self, row: usize, col: usize, s: &str) {
        let mut i = 0;
        for c in s.chars() {
            self.cells[row][col+i] = c;
            i += 1;
        }
    }

    pub fn new_blank(rows: usize, cols: usize) -> State {
        State {
            need_clear: false,
            rows: rows,
            cols: cols,
            cells: repeat_vec(repeat_vec(' ', cols), rows),
            cursor_row: 1,
            cursor_col: 1,
        }
    }

    pub fn new_real() -> State {
        let (rows, cols) = (30, 30);

        State {
            need_clear: true,
            rows: rows,
            cols: cols,
            cells: repeat_vec(repeat_vec('\0', cols), rows),
            cursor_row: 1,
            cursor_col: 1,
        }
    }
}

#[derive(Debug)]
enum Op {
    EraseEntireDisplay,
    Resize(usize, usize),
    Put(char),
    CursorPosition(usize, usize),
}

type Oplist = Vec<Op>;

fn code(ops: Oplist) -> Vec<u8> {
    let mut res = Vec::new();

    for op in ops {
        res.extend(match op {
            Op::EraseEntireDisplay => String::from("\x1b[2J").into_bytes(),
            Op::Resize(_w, _h) => String::new().into_bytes(),
            Op::Put(c) => format!("{}", c).into_bytes(),
            Op::CursorPosition(row, col) =>
                format!("\x1b[{};{}H", row, col).into_bytes(),
        });
    }

    res
}

// Creates an Oplist such that apply(ops, old) produces new.  Generally we want
// "ops" to be as performant as possible, although the performance model is
// unclear; generally a shorter code is always better.
fn spill(old: &State, new: &State) -> Oplist {
    let mut cursor_row = old.cursor_row;
    let mut cursor_col = old.cursor_col;
    let mut ops = Vec::new();

    if old.need_clear {
        ops.push(Op::EraseEntireDisplay);
        ops.push(Op::CursorPosition(1, 1));
    }

    if new.cols != old.cols || new.rows != old.rows {
        ops.push(Op::Resize(new.cols, new.rows))
    }

    for row in 1..new.rows+1 {
        for col in 1..new.cols+1 {
            let old_cell =
                if row <= old.rows && col <= old.cols {
                    old.cells[row - 1][col - 1]
                } else {
                    ' '
                };

            let new_cell = new.cells[row - 1][col - 1];

            if old_cell != new_cell {
                if cursor_col != col || cursor_row != row {
                    ops.push(Op::CursorPosition(row, col));
                    cursor_row = row;
                    cursor_col = col;
                }

                ops.push(Op::Put(new.cells[row - 1][col - 1]));
                cursor_col += 1;
                if cursor_col >= new.cols {
                    cursor_col -= new.cols;
                    cursor_row += 1;
                }
            }
        }
    }

    if cursor_col != new.cursor_col || cursor_row != new.cursor_row {
        ops.push(Op::CursorPosition(new.cursor_row, new.cursor_col));
    }

    ops
}

pub fn flush(real: &mut State, fake: State) -> std::io::Result<usize> {
    let n = std::io::stdout().write(code(spill(real, &fake)).as_slice())?;
    std::io::stdout().flush()?;
    *real = fake;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::{State, spill, Op, Oplist};

    fn apply(ops: Oplist, s: &mut State) {
        for op in ops {
            match op {
                Op::Resize(w, h) => {
                    if h < s.cells.len() {
                        s.cells.split_off(h);
                    }

                    while s.cells.len() < h {
                        s.cells.push(Vec::new());
                    }

                    for row in 1..h+1 {
                        while w > s.cells[row-1].len() {
                            s.cells[row-1].push(' ');
                        }

                        if s.cells[row-1].len() > w {
                            s.cells[row-1].split_off(w);
                        }
                    }

                    s.cols = w;
                    s.rows = h;
                },
                Op::Put(c) => {
                    s.cells[s.cursor_row - 1][s.cursor_col - 1] = c;
                    s.cursor_col += 1;
                    if s.cursor_col > s.cols {
                        s.cursor_col = 1;
                        s.cursor_row += 1;
                    }
                },
                Op::CursorPosition(row, col) => {
                    s.cursor_row = row;
                    s.cursor_col = col;
                },
            }
        }
    }

    fn assert_consistency(old: State, new: State) {
        let ops = spill(&old, &new);

        let mut applied = old.clone();
        apply(ops, &mut applied);

        assert_eq!(applied, new);
    }

    #[test]
    fn it_works() {
        assert_consistency(State {
            cols: 10,
            rows: 10,
            cells: vec![vec![' '; 10]; 10],
            cursor_row: 1,
            cursor_col: 1,
        }, State {
            cols: 8,
            rows: 10,
            cells: vec![vec![' '; 8]; 10],
            cursor_row: 5,
            cursor_col: 3,
        });
    }
}

