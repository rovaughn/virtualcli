
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct State {
    width: usize,
    height: usize,
    cells: Vec<Vec<char>>,
    cursor_row: usize,
    cursor_col: usize,
}

#[derive(Debug)]
enum Op {
    Resize(usize, usize),
    Put(char),
    CursorPosition(usize, usize),
}

type Oplist = Vec<Op>;

fn code(ops: Oplist) -> String {
    let mut res = String::new();

    for op in ops {
        res.push_str(&match op {
            Op::Resize(_w, _h) => String::new(),
            Op::Put(c) => format!("{}", c),
            Op::CursorPosition(row, col) => format!("\x1b[{};{}H", row, col),
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

    if new.width != old.width || new.height != old.height {
        ops.push(Op::Resize(new.width, new.height))
    }

    for row in 1..new.height+1 {
        for col in 1..new.width+1 {
            let old_cell =
                if row <= old.height && col <= old.width {
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
                if cursor_col >= new.width {
                    cursor_col -= new.width;
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

pub fn spill_code(old: &State, new: &State) -> String {
    code(spill(old, new))
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

                    s.width = w;
                    s.height = h;
                },
                Op::Put(c) => {
                    s.cells[s.cursor_row - 1][s.cursor_col - 1] = c;
                    s.cursor_col += 1;
                    if s.cursor_col > s.width {
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
            width: 10,
            height: 10,
            cells: vec![vec![' '; 10]; 10],
            cursor_row: 1,
            cursor_col: 1,
        }, State {
            width: 8,
            height: 10,
            cells: vec![vec![' '; 8]; 10],
            cursor_row: 5,
            cursor_col: 3,
        });
    }
}
