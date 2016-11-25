use std::io::Write;
use std::thread;
use std::sync::mpsc::{channel, Sender};
use std::mem::swap;

pub struct Screen<T> where T: Send {
    tx: Sender<ScreenCommand<T>>,
}

// TODO: Probably better to replace the pub fields with a public function
//       interface.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct State {
    cols: usize,
    rows: usize,
    cells: Vec<Vec<char>>,
    cursor_row: usize,
    cursor_col: usize,
}

enum ScreenCommand<T> where T: Send {
    ModifyState(Box<Fn(T) -> T + Send>),
    SetState(T),
    Die {},
}

impl<'a, T> Screen<T> where T: 'static + Send {
    pub fn new<F>(render: F, initial: T) -> Screen<T> where F: 'static + Fn(&T, &mut State) + Send {
        let (tx, rx) = channel();

        let screen = Screen {
            tx: tx,
        };

        thread::spawn(move || {
            std::io::stdout().write(code(&[Op::EraseEntireDisplay, Op::CursorPosition(1, 1)]).as_slice()).unwrap();
            let mut real_screen: State = State::new_real();
            let mut fake_screen = State::new_blank(30, 30);
            let mut state = initial;

            render(&state, &mut fake_screen);
            std::io::stdout().write(code(spill(&real_screen, &fake_screen).as_slice()).as_slice()).unwrap();
            std::io::stdout().flush().unwrap();
            swap(&mut real_screen, &mut fake_screen);

            loop {
                match rx.recv().unwrap() {
                ScreenCommand::ModifyState(f) => {
                    let new_state = f(state);
                    fake_screen.clear();
                    render(&new_state, &mut fake_screen);
                    std::io::stdout().write(code(spill(&real_screen, &fake_screen).as_slice()).as_slice()).unwrap();
                    std::io::stdout().flush().unwrap();
                    swap(&mut real_screen, &mut fake_screen);
                    state = new_state;
                },
                ScreenCommand::SetState(s) => { state = s; },
                ScreenCommand::Die{} => { return; },
                }
            }
        });

        screen
    }

    pub fn modify_state(&self, f: Box<Fn(T) -> T + Send>) {
        self.tx.send(ScreenCommand::ModifyState(f)).unwrap();
    }

    pub fn set_state(&self, s: T) {
        self.tx.send(ScreenCommand::SetState(s)).unwrap();
    }
}

impl<'a, T> Drop for Screen<T> where T: Send {
    // should probably wait til screen actually dies?  is it ok for a drop to
    // fail or block?
    fn drop(&mut self) {
        self.tx.send(ScreenCommand::Die{}).unwrap();
    }
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
    pub fn put(&mut self, row: usize, col: usize, c: char) {
        self.cells[row][col] = c;
    }

    pub fn write(&mut self, row: usize, col: usize, s: &str) {
        let mut i = 0;
        for c in s.chars() {
            self.cells[row][col+i] = c;
            i += 1;
        }
    }

    pub fn clear(&mut self) {
        self.cursor_row = 0;
        self.cursor_col = 0;
        for row in 0..self.rows {
            for col in 0..self.cols {
                self.cells[row][col] = ' ';
            }
        }
    }

    pub fn new_blank(rows: usize, cols: usize) -> State {
        State {
            rows: rows,
            cols: cols,
            cells: repeat_vec(repeat_vec(' ', cols), rows),
            cursor_row: 1,
            cursor_col: 1,
        }
    }

    // maybe this should include the code for initializing the screen.
    pub fn new_real() -> State {
        let (rows, cols) = (30, 30);

        State {
            rows: rows,
            cols: cols,
            cells: repeat_vec(repeat_vec(' ', cols), rows),
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

fn code(ops: &[Op]) -> Vec<u8> {
    let mut res = Vec::new();

    for op in ops {
        res.extend(match op {
        &Op::EraseEntireDisplay => String::from("\x1b[2J").into_bytes(),
        &Op::Resize(_w, _h) => String::new().into_bytes(),
        &Op::Put(c) => format!("{}", c).into_bytes(),
        &Op::CursorPosition(row, col) =>
            format!("\x1b[{};{}H", row + 1, col + 1).into_bytes(),
        });
    }

    res
}

// Creates a Vec<Op> such that apply(ops, old) produces new.  Generally we want
// "ops" to be as performant as possible, although the performance model is
// unclear; generally a shorter code is always better.
fn spill(old: &State, new: &State) -> Vec<Op> {
    let mut cursor_row = old.cursor_row;
    let mut cursor_col = old.cursor_col;
    let mut ops = Vec::new();

    if new.cols != old.cols || new.rows != old.rows {
        ops.push(Op::Resize(new.cols, new.rows))
    }

    for row in 0..new.rows {
        for col in 0..new.cols {
            let old_cell =
                if row < old.rows && col < old.cols {
                    old.cells[row][col]
                } else {
                    ' '
                };

            let new_cell = new.cells[row][col];

            if old_cell != new_cell {
                if cursor_row + 1 == row && col == 0 {
                    ops.push(Op::Put('\n'));
                } else if cursor_col != col || cursor_row != row {
                    ops.push(Op::CursorPosition(row, col));
                    cursor_row = row;
                    cursor_col = col;
                }

                ops.push(Op::Put(new_cell));
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

#[cfg(test)]
mod tests {
    use super::{State, spill, Op};

    fn apply(ops: &[Op], s: &mut State) {
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

