extern crate virtualcli;
extern crate rand;
use rand::distributions::{IndependentSample, Range};
use std::sync::Arc;

type Point = (usize, usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    North, South, East, West,
}

use Direction::*;

impl Direction {
    fn opposite(self) -> Self {
        match self {
            North => South,
            South => North,
            East  => West,
            West  => East,
        }
    }
}

#[derive(Debug)]
enum State {
    Lose,
    Win,
    Playing {
        direction: Direction,
        segments: Vec<Point>,
        food: Point,
    },
}

impl State {
    fn new() -> State {
        State::Playing {
            direction: South,
            segments: vec![(6, 5), (5, 5), (5, 4)],
            food: (4, 5),
        }
    }
}

fn tick(state: State) -> State {
    match state {
        State::Playing { mut segments, direction, food } => {
            let (head_row, head_col) = segments[0];

            let new_head = match direction {
                North => (head_row - 1, head_col),
                South => (head_row + 1, head_col),
                East  => (head_row, head_col + 1),
                West  => (head_row, head_col - 1),
            };

            for segment in &segments[0..segments.len()-1] {
                if new_head == *segment {
                    return State::Lose;
                }
            }

            if new_head == food {
                let mut rng = rand::thread_rng();

                if segments.len() == 20 {
                    return State::Win;
                }

                segments.insert(0, new_head);
                let food = (
                    Range::new(0, 8).ind_sample(&mut rng),
                    Range::new(0, 8).ind_sample(&mut rng),
                );

                State::Playing {
                    direction: direction,
                    segments: segments,
                    food: food,
                }
            } else {
                segments.insert(0, new_head);
                let i = segments.len() - 1;
                segments.remove(i);

                State::Playing {
                    direction: direction,
                    segments: segments,
                    food: food,
                }
            }
        },
        _ => state,

    }
}

fn main() {
    let screen = Arc::new(virtualcli::Screen::new(|state, screen| {
        let width = 10;
        let height = 10;

        for col in 1..width-1 {
            screen.put(0, col, '─');
            screen.put(height-1, col, '─');
        }

        for row in 1..height-1 {
            screen.put(row, 0, '│');
            screen.put(row, width-1, '│');
        }

        screen.put(0, 0, '┌');
        screen.put(height-1, 0, '└');
        screen.put(0, width-1, '┐');
        screen.put(height-1, width-1, '┘');

        match state {
        &State::Lose => {
            let mid_col = width/2-4;
            let mid_row = height/2;

            screen.write(mid_row, mid_col, "You lose!");
        },
        &State::Win => {
            let mid_col = width/2-4;
            let mid_row = height/2;

            screen.write(mid_row, mid_col, "You win!");
        },
        &State::Playing { ref segments, direction, food, ..  } => {
            let mut d = direction.clone();

            for i in 0..segments.len()-1 {
                let (y, x) = segments[i];
                let (ny, nx) = segments[i + 1];

                let nd =
                         if y > ny { South }
                    else if y < ny { North }
                    else if x > nx { East  }
                    else if x < nx { West  }
                    else           { panic!("impossible") };

                let c = match (d, nd) {
                ( East,  East) | ( West,  West) => '═',
                (North, North) | (South, South) => '║',
                ( East, North) | (South,  West) => '╔',
                ( West, North) | (South,  East) => '╗',
                (North,  West) | ( East, South) => '╚',
                (North,  East) | ( West, South) => '╝',
                _ => panic!("impossible direction pair {:?}", (d, nd)),
                };

                screen.put(y + 1, x + 1, c);

                d = nd;
            }

            let (food_row, food_col) = food;
            screen.put(food_row + 1, food_col + 1, '@');
        },
        }
    }, State::new()));

    let screen2 = screen.clone();
    std::thread::spawn(move || {
        use virtualcli::Key;
        let readkey = virtualcli::Readkey::new().unwrap();

        for key in readkey.receiver() {
            let dir = match key {
            Key::Up    => Some(North),
            Key::Down  => Some(South),
            Key::Right => Some(East),
            Key::Left  => Some(West),
            _          => None,
            };

            match dir {
            Some(d) =>
                screen2.modify_state(Box::new(move |state| {
                    match state {
                    State::Playing { direction, segments, food } =>
                        State::Playing { direction: d, segments: segments, food: food },
                    _ => state,
                    }
                })),
            _ => {},
            }
        }
    });

    loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        screen.modify_state(Box::new(|state| {
            tick(state)
        }));
    }
}

