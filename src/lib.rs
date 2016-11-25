extern crate libc;
use std::io::Write;
use std::thread;
use std::sync::mpsc;
use std::mem::swap;
use std::io::Read;

pub struct Readkey {
    saved_termios: libc::termios,
    rx: mpsc::Receiver<Key>,
}

use std::ffi::CString;

fn errno_message() -> String {
    unsafe {
        let errno = libc::__errno_location();
        let mut s = [0; 64];
        if libc::strerror_r(*errno, s.as_mut_ptr(), s.len()) != 0 {
            return String::from("strerror_r failed");
        }
        CString::from_raw(s.as_mut_ptr()).into_string().unwrap_or(String::from("could not convert strerror_r to string"))
    }
}

fn tcgetattr(fd: libc::c_int) -> Result<libc::termios, String> {
    let mut work = libc::termios {
        c_iflag: 0, c_oflag: 0, c_cflag: 0, c_lflag: 0, c_line: 0,
        c_cc: [0; 32],
        c_ispeed: 0, c_ospeed: 0,
    };

    unsafe {
        if libc::tcgetattr(fd, &mut work) == 0 {
            Ok(work)
        } else {
            Err(errno_message())
        }
    }
}

fn tcsetattr(fd: libc::c_int, optional_actions: libc::c_int, mut termios: libc::termios) -> Result<(), String> {
    unsafe {
        if libc::tcsetattr(fd, optional_actions, &mut termios) == 0 {
            Ok(())
        } else {
            Err(errno_message())
        }
    }
}

#[derive(Debug)]
pub enum Key {
    Chr(char),
    Del, End, Up, Down, Right, Left, Home,
}

impl Readkey {
    pub fn new() -> Result<Readkey, String> {
        let mut work = tcgetattr(libc::STDIN_FILENO)?;
        let saved_termios = work;

        work.c_lflag &= !libc::ECHO & !libc::ICANON;
        work.c_cc[libc::VMIN] = 0;
        work.c_cc[libc::VTIME] = 0;

        tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, work)?;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut stdin = std::io::stdin();
            let mut buf = Vec::new();
            let mut readbuf = [0];

            loop {
                match stdin.read(&mut readbuf) {
                Err(err) => panic!("{}", err),
                Ok(0)    => continue,
                Ok(1)    => buf.push(readbuf[0]),
                Ok(_)    => panic!("impossible"),
                }

                if buf.len() == 3 && buf[0] == ('\x1b' as u8) && buf[1] == ('[' as u8) {
                    tx.send(match buf[2] as char {
                    '3' => Key::Del,
                    '4' => Key::End,
                    'A' => Key::Up,
                    'B' => Key::Down,
                    'C' => Key::Right,
                    'D' => Key::Left,
                    'H' => Key::Home,
                    _   => { panic!("unknown combination \\[[\\x{:X}", buf[2]); },
                    }).unwrap();
                    buf.clear();
                } else if buf.len() == 1 && buf[0] as char == '\x1b' || buf.len() == 2 && buf[0] as char == '\x1b' && buf[1] as char == '[' {
                    // wait
                } else {
                    for c in &buf {
                        tx.send(Key::Chr(*c as char)).unwrap();
                    }
                    buf.clear();
                }
            }
        });

        Ok(Readkey {
            saved_termios: saved_termios,
            rx: rx,
        })
    }

    pub fn receiver(&self) -> &mpsc::Receiver<Key> {
        &self.rx
    }
}

impl Drop for Readkey {
    fn drop(&mut self) {
        tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, self.saved_termios).unwrap();
    }
}

pub struct Screen<T> where T: Send {
    tx: mpsc::SyncSender<ScreenCommand<T>>,
}

unsafe impl<T> Sync for Screen<T> where T: Send {}

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
        let (tx, rx) = mpsc::sync_channel(1);

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

