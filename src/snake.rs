extern crate virtualcli;
extern crate rand;
use rand::distributions::{IndependentSample, Range};

type Point = (usize, usize);

#[derive(Debug)]
enum Direction {
    North, South, East, West,
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
            direction: Direction::South,
            segments: vec![(5, 5)],
            food: (2, 2),
        }
    }
}

fn tick(state: State) -> State {
    match state {
        State::Playing { direction, mut segments, food } => {
            let mut rng = rand::thread_rng();
            let (head_row, head_col) = segments[0];

            let new_head = match direction {
                Direction::North => (head_row - 1, head_col),
                Direction::South => (head_row + 1, head_col),
                Direction::East  => (head_row, head_col + 1),
                Direction::West  => (head_row, head_col - 1),
            };

            for segment in &segments {
                if new_head == *segment {
                    return State::Lose;
                }
            }

            let x = Range::new(0, 4).ind_sample(&mut rng);
            let new_direction = match x {
                0 => Direction::North,
                1 => Direction::South,
                2 => Direction::East,
                3 => Direction::West,
                _ => panic!("impossible"),
            };

            if new_head == food {
                if segments.len() == 10 {
                    return State::Win;
                }

                segments.insert(0, new_head);
                let food = (
                    Range::new(0, 20).ind_sample(&mut rng),
                    Range::new(0, 20).ind_sample(&mut rng),
                );

                State::Playing {
                    direction: new_direction,
                    segments: segments,
                    food: food,
                }
            } else {
                segments[0] = new_head;

                State::Playing {
                    direction: new_direction,
                    segments: segments,
                    food: food,
                }
            }
        },
        _ => state,

    }
}

fn main() {
    let screen = virtualcli::Screen::new(|state, screen| {
        for col in 0..20 {
            screen.put(0, col, '-');
            screen.put(19, col, '-');
        }

        for row in 0..20 {
            screen.put(row, 0, '|');
            screen.put(row, 19, '|');
        }

        match state {
        &State::Lose => {
            let mid_col = 6;
            let mid_row = 10;

            screen.write(mid_row, mid_col, "You lose!");
        },
        &State::Win => {
            let mid_col = 6;
            let mid_row = 10;

            screen.write(mid_row, mid_col, "You win!");
        },
        &State::Playing { ref segments, food, ..  } => {
            for &(row, col) in segments {
                screen.put(row, col, '+');
            }

            let (food_row, food_col) = food;
            screen.put(food_row, food_col, '@');
        },
        }
    }, State::new());

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
        screen.modify_state(Box::new(|state| {
            tick(state)
        }));
    }
}

