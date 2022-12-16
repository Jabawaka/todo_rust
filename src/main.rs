use std::path::Path;
use std::env;
use std::{fs, fs::File};
use std::{io, io::Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::prelude::*;

use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Spans, Span},
    widgets::{
        Block, BorderType, Borders, Paragraph, Wrap,
    },
    Frame, Terminal,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use serde::{Deserialize, Serialize};

// ---- CONSTANTS ----
const BLINK_TIME: Duration = Duration::from_millis(400);


// ---- STRUCT AND ENUM DEFINITION ----
enum Event<I> {
    Input(I),
    Tick,
}

enum AppState {
    Display,
    EditTask,
}

#[derive(Serialize, Deserialize, Clone)]
struct Task {
    title: String,
    description: String,
    is_done: bool,
    is_active: bool,
    is_selected: bool,
    elapsed_time: Duration,
    created_on: DateTime<Utc>,
}

impl Task {
    fn get_time_str(&self) -> String {
        let mut time_str = String::from("");

        if self.elapsed_time.as_secs() < 60 {
            time_str.push_str("< 1 min");
        } else {
            let hours: u64 = (self.elapsed_time.as_secs() as f64 / 3600.0).floor() as u64;
            let mins: u64 = ((self.elapsed_time.as_secs() - hours * 3600) as f64 / 60.0).round() as u64;
            if hours > 0 {
                time_str.push_str(&hours.to_string());
                time_str.push_str(" h");
            }
            time_str.push_str(" ");
            time_str.push_str(&mins.to_string());
            time_str.push_str(" min");
        }

        time_str
    }

    fn toggle_active(&mut self, elapsed_time: Duration) {
        if self.is_active {
            self.elapsed_time += elapsed_time;
            self.is_active = false;
        } else {
            self.is_active = true;
        }
    }
}


struct App {
    db_path: String,
    last_event: Instant,
    tasks: Vec<Task>,
    state: AppState,

    desc_width_char: u16,

    first_string: String,
    blink_char: char,
    second_string: String,
    disp_string: String,
    cursor_pos: usize,
    cursor_shown: bool,
    last_blink: Instant,
}

impl App {
    fn new(path_to_db: &String) -> Result<App, Box<dyn std::error::Error>> {
        let db_content = fs::read_to_string(path_to_db)?;
        let mut parsed_tasks: Vec<Task> = serde_json::from_str(&db_content)?;

        for task in &mut parsed_tasks {
            task.is_selected = false;
        }

        if parsed_tasks.len() > 0 {
            parsed_tasks[0].is_selected = true;
        }

        Ok(App {
            db_path: path_to_db.clone(),
            last_event: Instant::now(),
            tasks: parsed_tasks.to_owned(),
            state: AppState::Display,

            desc_width_char: 0,

            first_string: String::from(""),
            blink_char: ' ',
            second_string: String::from(""),
            disp_string: String::from(""),
            cursor_pos: 0,
            cursor_shown: false,
            last_blink: Instant::now(),
        })
    }

    fn save_to_db(&mut self) {
        fs::write(&self.db_path, &serde_json::to_vec_pretty(&self.tasks).expect("DB should be writeable")).expect("DB should be writeable");
    }

    fn inc_sel_task(&mut self) {
        let mut index = 0;

        while index < self.tasks.len() - 1 {
            if self.tasks[index].is_selected {
                self.tasks[index].is_selected = false;
                self.tasks[index + 1].is_selected = true;
                break;
            }

            index += 1;
        }
    }

    fn dec_sel_task(&mut self) {
        let mut index = 1;

        while index < self.tasks.len() {
            if self.tasks[index].is_selected {
                self.tasks[index].is_selected = false;
                self.tasks[index - 1].is_selected = true;
            }

            index += 1;
        }
    }

    fn enter_edit(&mut self) {
        for task in &mut self.tasks {
            if task.is_selected {
                self.first_string = task.description.clone();
                self.blink_char = ' ';
                self.second_string = String::from("");

                self.last_blink = Instant::now();
                self.cursor_pos = self.first_string.chars().count();

                self.state = AppState::EditTask;
                break;
            }
        }
    }

    fn enter_display(&mut self) {
        for task in &mut self.tasks {
            if task.is_selected {
                task.description = self.first_string.clone();
                if self.second_string.chars().count() > 0
                {
                    task.description.push(self.blink_char);
                    task.description.push_str(&self.second_string);
                }

                self.state = AppState::Display;
                break;
            }
        }
    }

    fn activate_task(&mut self) {
        let now = Instant::now();

        for task in &mut self.tasks {
            // For the current active task do the ellapsed time and reset it
            if task.is_active {
                task.toggle_active(self.last_event.elapsed());
                self.last_event = now;
            } else if task.is_selected && !task.is_done {
                task.toggle_active(self.last_event.elapsed());
                self.last_event = now;
            }
        }
    }

    fn do_undo_task(&mut self) {
        let now = Instant::now();

        for task in &mut self.tasks {
            if task.is_selected {
                task.is_done = !task.is_done;

                if task.is_done && task.is_active {
                    task.toggle_active(self.last_event.elapsed());
                    self.last_event = now;
                }
            }
        }
    }

    fn get_cursor_pos(&self) -> (u16, u16) {
        let mut index = 0;
        let mut x = 0;
        let mut y = 0;

        while index < self.first_string.chars().count() {
            if self.first_string.chars().nth(index).unwrap() == '\n' {
                y += 1;
                x = 0;
            } else if x >= self.desc_width_char {
                y += 1;
                x -= self.desc_width_char;
            } else {
                x += 1;
            }
            index += 1;
        }

        (x, y)
    }

    fn set_cursor_pos(&mut self, des_x: u16, des_y: u16) {
        let mut curr_x = 0;
        let mut curr_y = 0;
        let mut index = 0;

        let mut cursor_set = false;

        let mut new_string = self.first_string.clone();
        if self.second_string.chars().count() > 0 {
            new_string.push(self.blink_char);
            new_string.push_str(&self.second_string);
        }

        while index < new_string.chars().count() {
            if curr_x >= des_x && curr_y == des_y {
                self.cursor_pos = index;

                self.first_string = new_string.drain(..self.cursor_pos).collect();
                self.blink_char = new_string.remove(0);
                self.second_string = new_string.clone();
                cursor_set = true;
                break;
            }

            if new_string.chars().nth(index).unwrap() == '\n' {
                if curr_y == des_y {
                    self.cursor_pos = index;

                    self.first_string = new_string.drain(..self.cursor_pos).collect();
                    self.blink_char = new_string.remove(0);
                    self.second_string = new_string.clone();
                    cursor_set = true;
                    break;
                } else {
                    curr_x = 0;
                    curr_y += 1;
                }
            } else if curr_x >= self.desc_width_char {
                curr_x -= self.desc_width_char;
                curr_y += 1;
            } else {
                curr_x += 1;
            }

            index += 1;
        }

        if !cursor_set {
            self.first_string = new_string.clone();
            self.blink_char = ' ';
            self.second_string = String::from("");

            self.cursor_pos = self.first_string.chars().count();
        }
    }

    fn dec_cursor(&mut self) {
        if self.first_string.chars().count() > 0 {
            self.second_string.insert(0, self.blink_char);
            self.blink_char = self.first_string.pop().unwrap();
            self.cursor_pos -= 1;
            self.cursor_shown = true;
            self.last_blink = Instant::now();
        }
    }

    fn inc_cursor(&mut self) {
        if self.second_string.chars().count() > 0 {
            self.first_string.push(self.blink_char);
            self.blink_char = self.second_string.remove(0);
            self.cursor_pos += 1;
            self.cursor_shown = true;
            self.last_blink = Instant::now();
        }
    }

    fn dec_line(&mut self) {
        if self.cursor_pos > 0 {
            let (x, y) = self.get_cursor_pos();

            if y > 0 {
                self.set_cursor_pos(x, y - 1);
            } else {
                self.set_cursor_pos(0, 0);
            }
        }
    }

    fn inc_line(&mut self) {
        let (x, y) = self.get_cursor_pos();

        self.set_cursor_pos(x, y + 1);
    }

    fn get_sel_task_info(&mut self) -> Option<Vec<Spans>> {
        for task in &self.tasks {
            if task.is_selected {
                let mut spans: Vec<Spans> = vec![];
                match self.state {
                    AppState::Display => {
                        let lines: Vec<&str> = task.description.split("\n").collect();

                        for line in lines {
                            spans.push(Spans::from(vec![Span::raw(line)]));
                        }
                    }
                    AppState::EditTask => {
                        let blink_char = if self.cursor_shown {
                            '_'
                        } else if self.blink_char == '\n' {
                            ' '
                        } else {
                            self.blink_char
                        };

                        if self.last_event.elapsed() > BLINK_TIME {
                            self.cursor_shown = !self.cursor_shown;
                            self.last_event = Instant::now();
                        }

                        self.disp_string = self.first_string.clone();
                        self.disp_string.push(blink_char);
                        if self.blink_char == '\n' {
                            self.disp_string.push('\n');
                        }
                        self.disp_string.push_str(&self.second_string);

                        let lines: Vec<&str> = self.disp_string.split("\n").collect();

                        for line in lines {
                            spans.push(Spans::from(vec![Span::raw(line)]));
                        }
                    }
                }
                return Some(spans);
            }
        }

        None
    }

    fn get_sel_task_title(&self) -> Option<String> {
        for task in &self.tasks {
            if task.is_selected {
                return Some(task.title.clone());
            }
        }

        None
    }

    fn delete_in_field(&mut self) {
        if self.first_string.chars().count() > 0 {
            self.first_string.pop();
            self.cursor_pos -= 1;
        }
    }

    fn type_in_field(&mut self, c: char) {
        self.first_string.push(c);
        self.cursor_pos += 1;
    }

    fn add_test_task(&mut self) {
        let task = Task {
            title: String::from("test"),
            description: String::from("This is a test"),
            is_done: false,
            is_active: false,
            is_selected: false,
            elapsed_time: Duration::new(5, 0),
            created_on: Utc::now(),
        };
        self.tasks.push(task.clone());
    }
}


// ---- MAIN FUNCTION ----
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ---- SET UP TERMINAL ----
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ---- PARSE INPUT ARGUMENTS AND CREATE APP ----
    let in_args: Vec<String> = env::args().collect();
    let num_args = in_args.len();
    let mut path_to_db = String::from("tasks.json");

    if num_args > 2 {
        println!("");
        println!("Too many arguments supplied! Either:");
        println!("  - Run the program with no args: this will create a local database file (tasks.json)");
        println!("  - Provide the file to be used as the first argument");
        println!("");
        panic!();
    } else if num_args == 1 {
        if !Path::new("tasks.json").exists() {
            let mut file = File::create("tasks.json")?;
            file.write_all(b"[]")?;
        }
    } else {
        path_to_db = in_args[1].clone();
    }

    let app = App::new(&path_to_db)?;

    // ---- RUN APP ----
    let res = run_app(&mut terminal, app);

    // ---- RESTORE TERMINAL ----
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

// ---- AUXILIARY FUNCTIONS ----
// Main run app function
fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<(), Box<dyn std::error::Error>> {
    // SET UP EVENT LOOP
    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("Polling should work!") {
                if let CEvent::Key(key) = event::read().expect("Should be able to read events!") {
                    tx.send(Event::Input(key)).expect("Should be able to send events!");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    // MAIN LOOP
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        match app.state {
            AppState::Display => {
                match rx.recv()? {
                    Event::Input(key) => {
                        match key.code {
                            KeyCode::Char('q') => {app.save_to_db(); return Ok(())},
                            KeyCode::Esc => return Ok(()),
                            KeyCode::Char('j') => app.inc_sel_task(),
                            KeyCode::Char('k') => app.dec_sel_task(),
                            KeyCode::Down => app.inc_sel_task(),
                            KeyCode::Up => app.dec_sel_task(),
                            KeyCode::Enter => app.activate_task(),
                            KeyCode::Char(' ') => app.do_undo_task(),
                            KeyCode::Char('a') => {app.add_test_task(); app.save_to_db()},
                            KeyCode::Char('e') => app.enter_edit(),
                            _ => {}
                        }
                    },
                    Event::Tick => {},
                }
            },
            AppState::EditTask => {
                match rx.recv()? {
                    Event::Input(key) => {
                        match key.code {
                            KeyCode::Esc => app.enter_display(),
                            KeyCode::Backspace => app.delete_in_field(),
                            KeyCode::Enter => app.type_in_field('\n'),
                            KeyCode::Left => app.dec_cursor(),
                            KeyCode::Right => app.inc_cursor(),
                            KeyCode::Up => app.dec_line(),
                            KeyCode::Down => app.inc_line(),
                            KeyCode::Char(c) => app.type_in_field(c),
                            _ => {}
                        }
                    },
                    Event::Tick => {},
                }
            },
        }
    }
}

// UI function
fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Min(2),
                Constraint::Length(3),
            ].as_ref(),
        ).split(size);

    let vsplit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(60),
            ]
        ).split(chunks[0]);

    app.desc_width_char = vsplit_layout[2].width - 2;

    let tasks: Vec<_> = app.tasks
        .iter()
        .map(|task| {
            let mut disp_string = String::from("");
            if task.is_done {
                disp_string.push_str("[X] ");
            } else {
                disp_string.push_str("[ ] ");
            }
            disp_string.push_str(&task.title);

            let mut fg_color = Color::Gray;
            if task.is_selected {
                fg_color = Color::Black;
            }
            if task.is_active {
                fg_color = Color::Green;
            }

            let mut style = Style::default().fg(fg_color);
            if task.is_selected {
                style = Style::default()
                    .bg(Color::Gray)
                    .fg(fg_color)
                    .add_modifier(Modifier::BOLD);
            }

            Spans::from(vec![Span::styled(disp_string, style)])
        })
        .collect();

    let tasks_duration: Vec<_> = app.tasks
        .iter()
        .map(|task| {
            let mut fg_color = Color::Gray;
            if task.is_selected {
                fg_color = Color::Black;
            }
            if task.is_active {
                fg_color = Color::Green;
            }

            let mut style = Style::default().fg(fg_color);
            if task.is_selected {
                style = Style::default()
                    .bg(Color::Gray)
                    .fg(fg_color)
                    .add_modifier(Modifier::BOLD);
            }

            Spans::from(vec![Span::styled(task.get_time_str(), style)])
        })
        .collect();

    let task_block = Paragraph::new(tasks)
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .style(Style::default())
            .title(" To Do ")
        );

    let task_dur_block = Paragraph::new(tasks_duration)
        .alignment(Alignment::Right)
        .block(
            Block::default()
            .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
            .style(Style::default())
        );

    let mut task_title = String::from(" ");
    task_title.push_str(&app.get_sel_task_title().unwrap_or_else(|| { String::from("") }));
    task_title.push_str(" ");

    let task_description = Paragraph::new(app.get_sel_task_info().unwrap_or_else(|| { vec![Spans::from(vec![Span::raw("")])] }))
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::ALL)
            .style(Style::default())
            .title(task_title)
        )
        .wrap(Wrap { trim: false });

    let instructions = Paragraph::new("' ' - Mark task as done | 'a' - Add task | 'e' - Edit task | enter - Mark task as active")
        .style(Style::default())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default())
                .border_type(BorderType::Double)
        );

    f.render_widget(task_block, vsplit_layout[0]);
    f.render_widget(task_dur_block, vsplit_layout[1]);
    f.render_widget(task_description, vsplit_layout[2]);
    f.render_widget(instructions, chunks[1]);
}