use console::{Key, Term};
use rand::Rng;
use std::char;
use std::fmt::Debug;
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
            jh: Some(tokio::spawn(Self::await_key_press())),
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

impl BaseRow {
    pub fn new(objects: Vec<bool>, object_label: char, environment_label: char) -> Self {
        Self {
            objects,
            object_label,
            environment_label,
        }
    }
    pub fn randomized_objects(object_label: char, environment_label: char) -> Self {
        let mut rng = rand::thread_rng();
        let mut objects = Vec::with_capacity(14);
        for _ in 0..14 {
            objects.push(rng.gen_bool(0.2));
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
    pub fn tick(&mut self) {
        self.tick_count += 1;
        if self.tick_count >= self.interval {
            self.tick_count = 0;
        }
            if self.direction {
                self.row
                    .objects
                    .insert(0, self.row.objects.clone().pop().unwrap());
            } else {
                self.row
                    .objects
                    .push(self.row.objects.clone().remove(0));
            }
    }
}

pub trait RowType: Debug {
    fn get_base_row(&self) -> &BaseRow;
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
    fn get_base_row(&self) -> &BaseRow {
        &self.dynamic_row.row
    }
    fn tick(&mut self) -> Option<bool> {
        Some(self.dynamic_row.direction)
    }
    fn check_position(&self, column_index: usize) -> Option<bool> {
        Some(self.dynamic_row.row.objects[column_index])
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
    fn get_base_row(&self) -> &BaseRow {
        &self.dynamic_row.row
    }
    fn tick(&mut self) -> Option<bool> {
        self.dynamic_row.tick();
        None
    }
    fn check_position(&self, column_index: usize) -> Option<bool> {
        Some(self.dynamic_row.row.objects[column_index])
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
    fn get_base_row(&self) -> &BaseRow {
        &self.baserow
    }
    fn tick(&mut self) -> Option<bool> {
        None
    }
    fn check_position(&self, column_index: usize) -> Option<bool> {
        Some(self.baserow.objects[column_index])
    }
}

pub struct GameState {
    gameboard: Vec<Box<dyn RowType>>,
    player: (usize, usize),
    keyreader: KeyReader,
    player_score: u32,
}

impl GameState {
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
        }
    }

    // Update stack will create random row, remove first row, and push new row
    pub fn create_random_row(previous_row: Option<&BaseRow>) -> Box<dyn RowType> {
        let mut rng = rand::thread_rng();
        let row_type = rng.gen_range(0..=2);
        let interval = rng.gen_range(1..=5);
        let direction = rng.gen_bool(0.5);
        let objects = BaseRow::randomized_objects(TREE, GRASS).objects;

        match row_type {
            0 => Box::new(Stream::new(objects, interval, direction)),
            1 => Box::new(Road::new(objects, interval, direction)),
            _ => Box::new(Grass::new(objects)),
        }
    }

    pub fn print_gameboard(&self) {
        let term = Term::stdout();
        term.clear_screen().unwrap();
        let player_row_index = self.player.1;

        for (row_index, row) in self.gameboard.iter().enumerate().rev() {
            for (col_index, &obj) in row.get_base_row().objects.iter().enumerate() {
                if row_index == player_row_index && col_index == self.player.0 {
                    print!("🐸");
                } else {
                    print!(
                        "{}",
                        if obj {
                            row.get_base_row().object_label
                        } else {
                            row.get_base_row().environment_label
                        }
                    );
                }
            }
            println!();
        }
        println!("Score: {}", self.player_score);
    }

    pub async fn tick(&mut self, key: Option<Key>) {
        self.gameboard.iter_mut().for_each(|row| {
            row.tick();
        });
            if key.is_some() && self.update_player(key.unwrap()).await {
            self.update_stack();
        } 
        // check the updated player position for legality
        // bounce back if needed
    }

    pub fn update_stack(&mut self) {
        self.gameboard.remove(0);
        self.gameboard.push(GameState::create_random_row(None));
        println!("Updated stack");
    }

    pub async fn run(&mut self) {
        loop {
            self.print_gameboard();
            if let Some(key) = self.keyreader.read_key().await {
                self.tick(Some(key)).await;
            } else {
                self.tick(None).await;
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn update_player(&mut self, key: Key) -> bool {
            match key {
                Key::Char('w') | Key::ArrowUp => {
                    if self.player.1 < 3 {
                        self.player.1 += 1;
                    }
                    return true;
                }
                Key::Char('s') | Key::ArrowDown => {
                    if self.player.1 > 0 {
                        self.player.1 -= 1;
                    }
                    return false;
                }
                Key::Char('a') | Key::ArrowLeft => {
                    if self.player.0 > 0 {
                        self.player.0 -= 1;
                    }
                    return false;
                }
                Key::Char('d') | Key::ArrowRight => {
                    if self.player.0 < 14 {
                        self.player.0 += 1;
                    }
                    return false;
                }
                _ => return false,
            }
    } 
}



#[tokio::main]
async fn main() {
    let mut game_state = GameState::new();
    game_state.run().await;
}
