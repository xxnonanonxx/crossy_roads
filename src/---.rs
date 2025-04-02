use console::{Key, Term};
use rand::Rng; //change
use std::{char, fmt::Debug, usize};
use tokio::time::{sleep, Duration};

const GRASS: char = '🟩';
const TREE: char = '🌲';
const ROAD: char = '⬛';
const CAR: char = '🚗';
const WATER: char = '🟦';
const PAD: char = '🟢';

#[derive(Debug)]
pub struct KeyReader {
    jh: Option<tokio::task::JoinHandle<Key>>,
}

impl KeyReader {
    pub fn new() -> KeyReader {
        KeyReader {
            jh: Some(tokio::spawn(async { Self::await_key_press().await })),
        }
    }

    async fn await_key_press() -> Key {
        let term = Term::stdout();
        term.read_key().unwrap()
    }

    pub async fn read_key(&mut self) -> Option<Key> {
        if let Some(handle) = self.jh.take() {
            match handle.await {
                Ok(key) => {
                    self.jh = Some(tokio::spawn(async { Self::await_key_press().await }));
                    Some(key)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct BaseRow {
    objects: Vec<bool>,
    object_label: char,
    environment_label: char,
}

// Refactor: BaseRow impl with stricter object limits
impl BaseRow {
    pub fn new(objects: Vec<bool>, object_label: char, environment_label: char) -> Self {
        Self {
            objects,
            object_label,
            environment_label,
        }
    }

    // Ensure logs in streams, limit objects (trees/cars) to 3 max
    pub fn randomized_objects(object_label: char, environment_label: char) -> Self {
        let mut rng = rand::thread_rng();
        let mut objects = Vec::with_capacity(14);
        for _ in 0..14 {
            objects.push(rng.gen_bool(0.2));
        }

        if environment_label == WATER && !objects.contains(&true) {
            let index = rng.gen_range(0..14);
            objects[index] = true; // Force a log
        }

        let mut true_count = objects.iter().filter(|&&x| x).count();
        while true_count > 3 {
            let index = rng.gen_range(0..14);
            if objects[index] {
                objects[index] = false; // Remove excess objects
                true_count -= 1;
            }
        }

        Self {
            objects,
            object_label,
            environment_label,
        }
    }
}

#[derive(Debug)]
pub struct DynamicRow {
    row: BaseRow,
    direction: bool,
    interval: u8,
    tick_count: u8,
}

impl DynamicRow {
    pub fn new(row: BaseRow, direction: bool, interval: u8) -> Self {
        Self {
            row,
            direction,
            interval,
            tick_count: 0,
        }
    }
    
    pub fn tick(&mut self) -> Option<bool> {
        let tick: u8 = self.tick_count;
        if tick == self.interval {
            self.tick_count = 0;
            Some(self.direction)
        } else {
            self.tick_count += 1;
            None
        }
    }

    pub fn update_row(&mut self) {
        let mut rng = rand::thread_rng();
        if self.direction {
            let first = rng.gen_bool(0.2); // New object enters from left
            for i in 0..self.row.objects.len() - 1 {
                self.row.objects[i] = self.row.objects[i + 1]; // Shift right
            }
            self.row.objects[13] = first; // Add new object at right
        } else {
            let last = rng.gen_bool(0.2); // New object enters from right
            for i in (1..self.row.objects.len()).rev() {
                self.row.objects[i] = self.row.objects[i - 1]; // Shift left
            }
            self.row.objects[0] = last; // Add new object at left
        }
    }
}

pub trait RowType: Debug {
    fn new(&self) -> &BaseRow;
    fn tick(&mut self) -> Option<bool>;
    fn check_position(&self, column_index: usize) -> Option<bool>;
}

#[derive(Debug)]
pub struct Stream {
    pub dynamic_row: DynamicRow,
}

impl Stream {
    pub fn new(objects: Vec<bool>, interval: u8, direction: bool) -> Self {
        Self {
            dynamic_row: DynamicRow::new(BaseRow::new(objects, PAD, WATER), direction, interval),
        }
    }
}

impl RowType for Stream {
    fn new(&self) -> &BaseRow {
        &self.dynamic_row.row
    }
    fn tick(&mut self) -> Option<bool> {
        if let Some(direction) = self.dynamic_row.tick() {
            self.dynamic_row.update_row();
            Some(direction)
        } else {
            None
        }
    }
    fn check_position(&self, column_index: usize) -> Option<bool> {
        if column_index < self.dynamic_row.row.objects.len() {
            Some(self.dynamic_row.row.objects[column_index])
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Road {
    pub dynamic_row: DynamicRow,
}

impl Road {
    pub fn new(objects: Vec<bool>, interval: u8, direction: bool) -> Self {
        Self {
            dynamic_row: DynamicRow::new(BaseRow::new(objects, CAR, ROAD), direction, interval),
        }
    }
}

impl RowType for Road {
    fn new(&self) -> &BaseRow {
        &self.dynamic_row.row
    }
    fn tick(&mut self) -> Option<bool> {
        if let Some(direction) = self.dynamic_row.tick() {
            self.dynamic_row.update_row();
            Some(direction)
        } else {
            None
        }
    }
    fn check_position(&self, column_index: usize) -> Option<bool> {
        if column_index < self.dynamic_row.row.objects.len() {
            Some(self.dynamic_row.row.objects[column_index])
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Grass {
    pub baserow: BaseRow,
}

impl Grass {
    pub fn new(objects: Vec<bool>) -> Self {
        Self {
            baserow: BaseRow::new(objects, TREE, GRASS),
        }
    }
}

impl RowType for Grass {
    fn new(&self) -> &BaseRow {
        &self.baserow
    }
    fn tick(&mut self) -> Option<bool> {
        None
    }
    fn check_position(&self, _column_index: usize) -> Option<bool> {
        if _column_index < self.baserow.objects.len() {
            Some(self.baserow.objects[_column_index])
        } else {
            None
        }
    }
}

pub struct GameState {
    gameboard: Vec<Box<dyn RowType>>,
    player: (usize, usize),
    keyreader: KeyReader,
    player_score: u32,
    game_over: bool,
    last_row_type: Option<u8>, // Track last row to limit streams
    game_over_reason: String,
}

impl GameState {
    // FEnsure player spawns free of trees, initialize game_over_reason
    pub fn new() -> Self {
        let mut bottom_row = BaseRow::randomized_objects(TREE, GRASS);
        bottom_row.objects[7] = false; 
        Self {
            gameboard: vec![
                Box::new(Grass::new(bottom_row.objects)),
                Box::new(Grass::new(BaseRow::randomized_objects(TREE, GRASS).objects)),
                GameState::create_random_row(None),
                GameState::create_random_row(None),
                GameState::create_random_row(None),
                GameState::create_random_row(None),
                GameState::create_random_row(None),
            ],
            player: (7, 0),
            keyreader: KeyReader::new(),
            player_score: 0,
            game_over: false,
            last_row_type: None,
            game_over_reason: String::new(), // Empty until game ends
        }
    }

    pub fn create_random_row(last_row_type: Option<u8>) -> Box<dyn RowType> {
        let mut rng = rand::thread_rng();
        let row_type = if last_row_type == Some(0) {
            rng.gen_range(1..3) // Avoid Stream if last was Stream
        } else {
            rng.gen_range(0..3)
        };
        let new_row: Box<dyn RowType> = match row_type {
            0 => Box::new(Stream::new(
                BaseRow::randomized_objects(PAD, WATER).objects,
                2,
                rng.gen_bool(0.5),
            )) as Box<dyn RowType>,
            1 => Box::new(Road::new(
                BaseRow::randomized_objects(CAR, ROAD).objects,
                2,
                rng.gen_bool(0.5),
            )) as Box<dyn RowType>,
            _ => Box::new(Grass::new(BaseRow::randomized_objects(TREE, GRASS).objects)) as Box<dyn RowType>,
        };
        new_row
    }

    pub fn print_gameboard(&self) {
        let term = Term::stdout();
        term.clear_screen().unwrap();
        let player_row_index = self.player.1;
        for (row_index, row) in self.gameboard.iter().enumerate().rev() {
            for (col_index, &obj) in row.new().objects.iter().enumerate() {
                if row_index == player_row_index && col_index == self.player.0 {
                    print!("🐸");
                } else {
                    print!(
                        "{}",
                        if obj {
                            row.new().object_label
                        } else {
                            row.new().environment_label
                        }
                    );
                }
            }
            println!();
        }
        println!("Score: {}", self.player_score);
    }

    pub async fn run(&mut self) {
        let term = Term::stdout();
        loop {
            self.print_gameboard();

            // Move objects every frame, not tied to keypress
            for row in self.gameboard.iter_mut() {
                row.tick();
            }

            self.update_player().await;

            let row = &self.gameboard[self.player.1];
            let base_row = row.new();
            let pos_check = row.check_position(self.player.0);
            if base_row.environment_label == ROAD && pos_check == Some(true) {
                self.game_over = true;
                self.game_over_reason = "Hit by a car".to_string();
            } else if base_row.environment_label == WATER && pos_check == Some(false) {
                self.game_over = true;
                self.game_over_reason = "Drowned in water".to_string();
            }

            if self.game_over {
                term.write_line(&format!("Game Over: {}", self.game_over_reason)).unwrap();
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn update_player(&mut self) {
        if let Some(key) = self.keyreader.read_key().await {
            let next_position: (usize, usize) = match key {
                Key::Char('w') | Key::ArrowUp => {
                    if self.player.1 < 3 {
                        (self.player.0, self.player.1 + 1)
                    } else if self.player.1 == 3 {
                        self.gameboard.remove(0);
                        let new_row = GameState::create_random_row(self.last_row_type);
                        self.gameboard.push(new_row);
                        // Update last_row_type based on new row at top (index 6)
                        self.last_row_type = match self.gameboard[6].new().environment_label {
                            WATER => Some(0),
                            ROAD => Some(1),
                            _ => Some(2),
                        };
                        self.player_score += 1;
                        (self.player.0, self.player.1)
                    } else {
                        (self.player.0, self.player.1)
                    }
                }
                Key::Char('a') | Key::ArrowLeft if self.player.0 > 0 => {
                    (self.player.0 - 1, self.player.1)
                }
                Key::Char('s') | Key::ArrowDown if self.player.1 > 0 => {
                    (self.player.0, self.player.1 - 1)
                }
                Key::Char('d') | Key::ArrowRight if self.player.0 < 13 => {
                    (self.player.0 + 1, self.player.1)
                }
                Key::Escape => std::process::exit(0),
                _ => (self.player.0, self.player.1),
            };

            // Check obstacles (trees block movement)
            if let Some(true) = self.gameboard[next_position.1].check_position(next_position.0) {
                return;
            }

            self.player = next_position;
        }
    }
}

#[tokio::main]
async fn main() {
    let mut game_state = GameState::new();
    game_state.run().await;
}