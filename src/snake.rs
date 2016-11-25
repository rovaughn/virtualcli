extern crate virtualcli;
extern crate rand;
use rand::distributions::{IndependentSample, Range};

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

fn choose<'a, R, T>(rng: &mut R, choices: &'a [T]) -> &'a T where R: rand::Rng {
    let samples = rand::sample(rng, choices, 1);
    samples[0]
}

fn tick(state: State) -> State {
    match state {
        State::Playing { mut segments, direction, food } => {
            let mut rng = rand::thread_rng();
            let (head_row, head_col) = segments[0];

            let new_head = match direction {
                Direction::North => (head_row - 1, head_col),
                Direction::South => (head_row + 1, head_col),
                Direction::East  => (head_row, head_col + 1),
                Direction::West  => (head_row, head_col - 1),
            };

            for segment in &segments[0..segments.len()-1] {
                if new_head == *segment {
                    return State::Lose;
                }
            }

            let new_direction = {
                let mut dirs = vec![North, South, East, West];

                for i in 0..dirs.len() {
                    if dirs[i] == direction.opposite() {
                        dirs.remove(i);
                        break;
                    }
                }

                rand::sample(&mut rng, dirs, 1)[0]
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
                segments.insert(0, new_head);
                let i = segments.len() - 1;
                segments.remove(i);

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
                (Direction::East, Direction::East) | (Direction::West, Direction::West) => '═',
                (Direction::North, Direction::North) | (Direction::South, Direction::South) => '║',
                (Direction::East, Direction::North) | (Direction::South, Direction::West)  => '╔',
                (Direction::West, Direction::North) | (Direction::South, Direction::East) => '╗',
                (Direction::North, Direction::West) | (Direction::East, Direction::South) => '╚',
                (Direction::North, Direction::East) | (Direction::West, Direction::South) => '╝',
                _ => panic!("impossible direction pair {:?}", (d, nd)),
                };

                screen.put(y, x, c);

                d = nd;
            }

            let (food_row, food_col) = food;
            screen.put(food_row, food_col, '@');
        },
        }
    }, State::new());

    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        screen.modify_state(Box::new(|state| {
            tick(state)
        }));
    }
}

