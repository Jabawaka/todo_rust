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
        Block, BorderType, Borders, Paragraph, Tabs, Wrap,
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

#[derive(PartialEq)]
enum AppState {
    Display,
    EditTask,
    Stats,
    Settings,
    EditSetting,
}

#[derive(PartialEq)]
enum EditSettingField {
    Split,
    NormalFg,
    NormalBg,
    SelectionFg,
    SelectionBg,
    Active,
    Title
}

impl From<AppState> for usize {
    fn from(input: AppState) -> usize {
        match input {
            AppState::Display     => 0,
            AppState::EditTask    => 0,
            AppState::Stats       => 1,
            AppState::Settings    => 2,
            AppState::EditSetting => 2,
        }
    }
}

#[derive(PartialEq)]
enum EditField {
    Title,
    Description,
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

    fn toggle_active(&mut self) {
        if self.is_active {
            self.is_active = false;
        } else {
            self.is_active = true;
        }
    }
}


#[derive(Serialize, Deserialize)]
struct Settings {
    // Layout
    is_horizontal: bool,

    // Styles
    default: Style,
    highlight: Style,
    active_normal: Style,
    active_highlight: Style,
    title: Style,

    // Colours for changing
    normal_fg_colour: Color,
    normal_bg_colour: Color,
    select_fg_colour: Color,
    select_bg_colour: Color,
    active_fg_colour: Color,
    title_fg_colour: Color,
}

fn colour_to_string(colour: Color) -> String {
    match colour {
        Color::White => String::from("White"),
        Color::Black => String::from("Black"),
        Color::Cyan => String::from("Cyan"),
        Color::Green => String::from("Green"),
        Color::Blue => String::from("Blue"),
        _ => String::from("Unknown"),
    }
}


struct App {
    // App state
    db_path: String,
    last_event: Instant,
    tasks: Vec<Task>,
    state: AppState,
    edit_field: EditField,
    edit_setting: EditSettingField,

    // Displaying variables
    desc_width_char: u16,

    // Editing variables
    first_string: String,
    blink_char: char,
    second_string: String,
    disp_string: String,
    cursor_pos: usize,
    cursor_shown: bool,
    last_blink: Instant,

    // Settings
    settings: Settings,
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

        let settings_content = fs::read_to_string("settings.json")?;
        let settings: Settings = serde_json::from_str(&settings_content)?;

        Ok(App {
            db_path: path_to_db.clone(),
            last_event: Instant::now(),
            tasks: parsed_tasks.to_owned(),
            state: AppState::Display,
            edit_field: EditField::Description,
            edit_setting: EditSettingField::Split,

            desc_width_char: 0,

            first_string: String::from(""),
            blink_char: '\t',
            second_string: String::from(""),
            disp_string: String::from(""),
            cursor_pos: 0,
            cursor_shown: false,
            last_blink: Instant::now(),

            settings: settings,
        })
    }

    fn save_to_db(&mut self) {
        fs::write(&self.db_path, &serde_json::to_vec_pretty(&self.tasks).expect("DB should be writeable")).expect("DB should be writeable");
    }

    fn save_settings(&mut self) {
        fs::write("settings.json", &serde_json::to_vec_pretty(&self.settings).expect("Settings should be writeable")).expect("Settings should be writeable");
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

    fn enter_edit(&mut self, edit: EditField) {
        for task in &mut self.tasks {
            if task.is_selected {
                match edit {
                    EditField::Title => self.first_string = task.title.clone(),
                    EditField::Description => self.first_string = task.description.clone(),
                }
                self.blink_char = '\t';
                self.second_string = String::from("");

                self.last_blink = Instant::now();
                self.cursor_pos = self.first_string.chars().count();

                self.state = AppState::EditTask;
                self.edit_field = edit;
                break;
            }
        }
    }

    fn enter_display(&mut self) {
        for task in &mut self.tasks {
            if task.is_selected {
                match self.edit_field {
                    EditField::Title => {
                        task.title = self.first_string.clone();
                        if self.second_string.chars().count() > 0
                        {
                            task.title.push(self.blink_char);
                            task.title.push_str(&self.second_string);
                        }

                        task.title.retain(|c| c != '\t');
                    },
                    EditField::Description => {
                        task.description = self.first_string.clone();
                        if self.second_string.chars().count() > 0
                        {
                            task.description.push(self.blink_char);
                            task.description.push_str(&self.second_string);
                        }
                    }
                }
                self.state = AppState::Display;
            }
        }
    }

    fn change_field(&mut self) {
        for task in &mut self.tasks {
            if task.is_selected {
                match self.edit_field {
                    EditField::Title => {
                        task.title = self.first_string.clone();
                        if self.second_string.chars().count() > 0
                        {
                            task.title.push(self.blink_char);
                            task.title.push_str(&self.second_string);
                        }
                        self.first_string = task.description.clone();
                        self.blink_char = '\t';
                        self.second_string = String::from("");

                        self.last_blink = Instant::now();
                        self.cursor_pos = self.first_string.chars().count();

                        self.edit_field = EditField::Description;
                    },
                    EditField::Description => {
                        task.description = self.first_string.clone();
                        if self.second_string.chars().count() > 0
                        {
                            task.description.push(self.blink_char);
                            task.description.push_str(&self.second_string);
                        }
                        self.first_string = task.title.clone();
                        self.blink_char = '\t';
                        self.second_string = String::from("");

                        self.last_blink = Instant::now();
                        self.cursor_pos = self.first_string.chars().count();

                        self.edit_field = EditField::Title;
                    },
                }
            }
        }
    }

    fn update_times(&mut self) {
        for task in &mut self.tasks {
            if task.is_active {
                task.elapsed_time += self.last_event.elapsed();
                self.last_event = Instant::now();
            }
        }
    }

    fn activate_task(&mut self) {
        for task in &mut self.tasks {
            // For the current active task do the ellapsed time and reset it
            if task.is_active {
                task.toggle_active();
            } else if task.is_selected && !task.is_done {
                task.toggle_active();
            }
        }
    }

    fn do_undo_task(&mut self) {
        for task in &mut self.tasks {
            if task.is_selected {
                task.is_done = !task.is_done;

                if task.is_done && task.is_active {
                    task.toggle_active();
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
                if self.state == AppState::EditTask && self.edit_field == EditField::Description {
                    if self.last_blink.elapsed() > BLINK_TIME {
                        self.cursor_shown = !self.cursor_shown;
                        self.last_blink = Instant::now();
                    }

                    let blink_char = if self.cursor_shown {
                        '_'
                    } else if self.blink_char == '\n' {
                        ' '
                    } else {
                        self.blink_char
                    };

                    self.disp_string = String::from("\n");
                    self.disp_string.push_str(&self.first_string);
                    self.disp_string.push(blink_char);
                    if self.blink_char == '\n' {
                        self.disp_string.push('\n');
                    }
                    self.disp_string.push_str(&self.second_string);

                    let lines: Vec<&str> = self.disp_string.split("\n").collect();

                    for line in lines {
                        spans.push(Spans::from(vec![Span::raw(line)]));
                    }
                } else {
                    self.disp_string = String::from("\n");
                    self.disp_string.push_str(&task.description);
                    let lines: Vec<&str> = self.disp_string.split("\n").collect();

                    for line in lines {
                        spans.push(Spans::from(vec![Span::raw(line)]));
                    }
                }

                return Some(spans);
            }
        }

        None
    }

    fn get_sel_task_title(&mut self) -> Option<String> {
        for task in &self.tasks {
            if task.is_selected {
                if self.state == AppState::EditTask && self.edit_field == EditField::Title {
                    if self.last_blink.elapsed() > BLINK_TIME {
                        self.cursor_shown = !self.cursor_shown;
                        self.last_blink = Instant::now();
                    }

                    let blink_char = if self.cursor_shown {
                        '_'
                    } else if self.blink_char == '\n' {
                        ' '
                    } else {
                        self.blink_char
                    };

                    self.disp_string = self.first_string.clone();
                    self.disp_string.push(blink_char);
                    self.disp_string.push_str(&self.second_string);

                    return Some(self.disp_string.clone());
                } else {
                    return Some(task.title.clone());
                }
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

    fn add_task(&mut self) {
        for task in &mut self.tasks {
            task.is_selected = false;
        }
        let task = Task {
            title: String::from(""),
            description: String::from(""),
            is_done: false,
            is_active: false,
            is_selected: true,
            elapsed_time: Duration::new(0, 0),
            created_on: Utc::now(),
        };
        self.tasks.push(task.clone());

        self.enter_edit(EditField::Title);
    }

    fn del_task(&mut self) {
        let mut index = 0;
        while index < self.tasks.len() {
            if self.tasks[index].is_selected {
                self.tasks.remove(index);

                if self.tasks.len() > 0 {
                    if index < self.tasks.len() {
                        self.tasks[index].is_selected = true;
                    } else {
                        self.tasks[index - 1].is_selected = true;
                    }
                }
                break;
            }
            index += 1;
        }
    }

    fn inc_setting_selection(&mut self) {
        match self.edit_setting {
            EditSettingField::Split => self.edit_setting = EditSettingField::NormalFg,
            EditSettingField::NormalFg => self.edit_setting = EditSettingField::NormalBg,
            EditSettingField::NormalBg => self.edit_setting = EditSettingField::SelectionFg,
            EditSettingField::SelectionFg => self.edit_setting = EditSettingField::SelectionBg,
            EditSettingField::SelectionBg => self.edit_setting = EditSettingField::Active,
            EditSettingField::Active => self.edit_setting = EditSettingField::Title,
            _ => {},
        }
    }

    fn dec_setting_selection(&mut self) {
        match self.edit_setting {
            EditSettingField::NormalFg => self.edit_setting = EditSettingField::Split,
            EditSettingField::NormalBg => self.edit_setting = EditSettingField::NormalFg,
            EditSettingField::SelectionFg => self.edit_setting = EditSettingField::NormalBg,
            EditSettingField::SelectionBg => self.edit_setting = EditSettingField::SelectionFg,
            EditSettingField::Active => self.edit_setting = EditSettingField::SelectionBg,
            EditSettingField::Title => self.edit_setting = EditSettingField::Active,
            _ => {},
        }
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

        app.update_times();

        match app.state {
            AppState::Display => {
                match rx.recv()? {
                    Event::Input(key) => {
                        match key.code {
                            KeyCode::Char('q') => {app.save_to_db(); app.save_settings(); return Ok(())},
                            KeyCode::Esc => {app.save_to_db(); app.save_settings(); return Ok(())},
                            KeyCode::Char('s') => app.save_to_db(),
                            KeyCode::Char('j') => app.inc_sel_task(),
                            KeyCode::Char('k') => app.dec_sel_task(),
                            KeyCode::Down => app.inc_sel_task(),
                            KeyCode::Up => app.dec_sel_task(),
                            KeyCode::Enter => app.activate_task(),
                            KeyCode::Char(' ') => app.do_undo_task(),
                            KeyCode::Char('a') => app.add_task(),
                            KeyCode::Char('d') => app.del_task(),
                            KeyCode::Char('e') => app.enter_edit(EditField::Description),
                            KeyCode::Tab => app.state = AppState::Stats,
                            KeyCode::BackTab => app.state = AppState::Settings,
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
                            KeyCode::Tab => app.change_field(),
                            _ => {}
                        }
                    },
                    Event::Tick => {},
                }
            },
            AppState::Stats => {
                match rx.recv()? {
                    Event::Input(key) => {
                        match key.code {
                            KeyCode::Char('q') => {app.save_to_db(); app.save_settings(); return Ok(())},
                            KeyCode::Esc => {app.save_to_db(); app.save_settings(); return Ok(())},
                            KeyCode::Tab => app.state = AppState::Settings,
                            KeyCode::BackTab => app.state = AppState::Display,
                            _ => {}
                        }
                    },
                    Event::Tick => {},
                }
            },
            AppState::Settings => {
                match rx.recv()? {
                    Event::Input(key) => {
                        match key.code {
                            KeyCode::Char('q') => {app.save_to_db(); app.save_settings(); return Ok(())},
                            KeyCode::Esc => {app.save_to_db(); app.save_settings(); return Ok(())},
                            KeyCode::Char('h') => app.state = AppState::Display,
                            KeyCode::Char('l') => app.state = AppState::Display,
                            KeyCode::Up => app.dec_setting_selection(),
                            KeyCode::Down => app.inc_setting_selection(),
                            KeyCode::Tab => app.state = AppState::Display,
                            KeyCode::BackTab => app.state = AppState::Stats,
                            _ => {}
                        }
                    },
                    Event::Tick => {},
                }
            },
            AppState::EditSetting => {},
        }
    }
}

// UI function
fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    match app.state {
        AppState::Display => render_tasks(f, app),
        AppState::EditTask => render_tasks(f, app),
        AppState::Stats => {},
        AppState::Settings => render_settings(f, app),
        AppState::EditSetting => render_settings(f, app),
    }
}


// Render tasks screen
fn render_tasks<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(2),
                Constraint::Min(2),
                Constraint::Length(4),
            ].as_ref(),
        ).split(size);

    let vsplit_layout = if app.settings.is_horizontal {
        Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(chunks[1])
    } else {
        Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(chunks[1])
    };

    let hsplit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]
        ).split(vsplit_layout[0]);

    // Capture displaying variables
    app.desc_width_char = vsplit_layout[1].width - 2;
    let default_style = app.settings.default.clone();

    // Render menu
    let menu_titles = vec!["Tasks", "Stats", "Settings"];
    let menu = menu_titles
        .iter()
        .map(|t| {
            let (first, rest) = t.split_at(1);
            Spans::from(vec![
                Span::styled(
                    first,
                    app.settings.title,
                ),
                Span::styled(rest, default_style),
            ])
        })
        .collect();

    let tabs = Tabs::new(menu)
        .select(AppState::Display.into())
        .block(Block::default().borders(Borders::BOTTOM).border_type(BorderType::Double))
        .style(default_style)
        .highlight_style(app.settings.title)
        .divider(Span::styled("|", default_style));

    // Render tasks information
    let mut tasks: Vec<_> = app.tasks
        .iter()
        .map(|task| {
            let mut disp_string = String::from("");
            if task.is_done {
                disp_string.push_str("[X] ");
            } else {
                disp_string.push_str("[ ] ");
            }
            disp_string.push_str(&task.title);

            let mut style = app.settings.default;
            if task.is_selected {
                if task.is_active {
                    style = app.settings.active_highlight;
                } else {
                    style = app.settings.highlight;
                }
            } else if task.is_active {
                style = app.settings.active_normal;
            }

            Spans::from(vec![Span::styled(disp_string, style)])
        })
        .collect();

    tasks.insert(0, Spans::from(vec![Span::styled(String::from(""), default_style)]));

    let mut tasks_duration: Vec<_> = app.tasks
        .iter()
        .map(|task| {
            let mut style = app.settings.default;
            if task.is_selected {
                if task.is_active {
                    style = app.settings.active_highlight;
                } else {
                    style = app.settings.highlight;
                }
            } else if task.is_active {
                style = app.settings.active_normal;
            }

            Spans::from(vec![Span::styled(task.get_time_str(), style)])
        })
        .collect();

    tasks_duration.insert(0, Spans::from(vec![Span::styled(String::from(""), default_style)]));

    let task_block = Paragraph::new(tasks)
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .style(default_style)
            .title(" To Do ")
        );

    let task_dur_block = Paragraph::new(tasks_duration)
        .alignment(Alignment::Right)
        .block(
            Block::default()
            .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
            .style(default_style)
        );

    let mut task_title = String::from(" ");
    task_title.push_str(&app.get_sel_task_title().unwrap_or_else(|| { String::from("") }));
    task_title.push_str(" ");

    let task_description = Paragraph::new(app.get_sel_task_info().unwrap_or_else(|| { vec![Spans::from(vec![Span::raw("")])] }))
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::ALL)
            .style(default_style)
            .title(task_title)
        )
        .wrap(Wrap { trim: false });

    // Render instructions
    let instructions = Paragraph::new("' ' - Mark task as done | 'a' - Add task         | 'e' - Edit task        | 'd' - Delete task      \n'j' - Go up             | 'k' - Go down          | 'h' - Go to left tab   | 'l' - Go to right tab  \n'c' - Archive tasks     | 's' - Save tasks       | enter - Activate task  | esc,'q' - Quit         ")
        .style(default_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(default_style)
                .border_type(BorderType::Double)
        );

    f.render_widget(tabs, chunks[0]);
    f.render_widget(task_block, hsplit_layout[0]);
    f.render_widget(task_dur_block, hsplit_layout[1]);
    f.render_widget(task_description, vsplit_layout[1]);
    f.render_widget(instructions, chunks[2]);
}


// Render settings
fn render_settings<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(2),
                Constraint::Min(2),
                Constraint::Length(4),
            ].as_ref(),
        ).split(size);

    let vsplit_layout = if app.settings.is_horizontal {
        Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(chunks[1])
    } else {
        Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(chunks[1])
    };

    let hsplit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(vsplit_layout[0]);

    // Capture displaying variables
    let default_style = app.settings.default.clone();

    // Render menu
    let menu_titles = vec!["Tasks", "Stats", "Settings"];
    let menu = menu_titles
        .iter()
        .map(|t| {
            let (first, rest) = t.split_at(1);
            Spans::from(vec![
                Span::styled(
                    first,
                    app.settings.title,
                ),
                Span::styled(rest, default_style),
            ])
        })
        .collect();

    let tabs = Tabs::new(menu)
        .select(AppState::Settings.into())
        .block(Block::default().borders(Borders::BOTTOM).border_type(BorderType::Double))
        .style(default_style)
        .highlight_style(app.settings.title)
        .divider(Span::styled("|", default_style));

    // Render settings
    let settings_sections = Paragraph::new(vec![
        Spans::from(vec![Span::styled("Layout", app.settings.default.add_modifier(Modifier::UNDERLINED))]),
        Spans::from(vec![Span::styled("", app.settings.default)]),
        Spans::from(vec![
            Span::styled("  ", app.settings.default),
            Span::styled(
                "Split",
                if app.edit_setting == EditSettingField::Split { app.settings.highlight } else { app.settings.default }
            )]),
        Spans::from(vec![Span::styled("", app.settings.default)]),
        Spans::from(vec![Span::styled("Task colours", app.settings.default.add_modifier(Modifier::UNDERLINED))]),
        Spans::from(vec![Span::styled("", app.settings.default)]),
        Spans::from(vec![
            Span::styled("  ", app.settings.default),
            Span::styled(
                "Main foreground colour",
                if app.edit_setting == EditSettingField::NormalFg { app.settings.highlight } else { app.settings.default }
            )]),
        Spans::from(vec![
            Span::styled("  ", app.settings.default),
            Span::styled(
                "Main background colour",
                if app.edit_setting == EditSettingField::NormalBg { app.settings.highlight } else { app.settings.default }
            )]),
        Spans::from(vec![
            Span::styled("  ", app.settings.default),
            Span::styled(
                "Selected foreground colour",
                if app.edit_setting == EditSettingField::SelectionFg { app.settings.highlight } else { app.settings.default }
            )]),
        Spans::from(vec![
            Span::styled("  ", app.settings.default),
            Span::styled(
                "Selected background colour",
                if app.edit_setting == EditSettingField::SelectionBg { app.settings.highlight } else { app.settings.default }
            )]),
        Spans::from(vec![
            Span::styled("  ", app.settings.default),
            Span::styled(
                "Active task colour",
                if app.edit_setting == EditSettingField::Active { app.settings.highlight } else { app.settings.default }
            )]),
        Spans::from(vec![
            Span::styled("  ", app.settings.default),
            Span::styled(
                "Title colour",
                if app.edit_setting == EditSettingField::Title { app.settings.highlight } else { app.settings.default }
            )]),
    ])
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
                .style(default_style)
        );

    let settings_values = Paragraph::new(vec![
        Spans::from(vec![Span::styled("", app.settings.default)]),
        Spans::from(vec![Span::styled("", app.settings.default)]),
        Spans::from(vec![
            Span::styled(
                if app.settings.is_horizontal { "Horizontal" } else { "Vertical" },
                if app.edit_setting == EditSettingField::Split { app.settings.highlight } else { app.settings.default }),
            Span::styled("    ", app.settings.default)]),
        Spans::from(vec![Span::styled("", app.settings.default)]),
        Spans::from(vec![Span::styled("", app.settings.default)]),
        Spans::from(vec![Span::styled("", app.settings.default)]),
        Spans::from(vec![
            Span::styled(colour_to_string(app.settings.normal_fg_colour),
            if app.edit_setting == EditSettingField::NormalFg { app.settings.highlight } else { app.settings.default }),
            Span::styled("    ", app.settings.default)]),
        Spans::from(vec![
            Span::styled(colour_to_string(app.settings.normal_bg_colour),
            if app.edit_setting == EditSettingField::NormalBg { app.settings.highlight } else { app.settings.default }),
            Span::styled("    ", app.settings.default)]),
        Spans::from(vec![
            Span::styled(colour_to_string(app.settings.select_fg_colour),
            if app.edit_setting == EditSettingField::SelectionFg { app.settings.highlight } else { app.settings.default }),
            Span::styled("    ", app.settings.default)]),
        Spans::from(vec![
            Span::styled(colour_to_string(app.settings.select_bg_colour),
            if app.edit_setting == EditSettingField::SelectionBg { app.settings.highlight } else { app.settings.default }),
            Span::styled("    ", app.settings.default)]),
        Spans::from(vec![
            Span::styled(colour_to_string(app.settings.active_fg_colour),
            if app.edit_setting == EditSettingField::Active { app.settings.highlight } else { app.settings.default }),
            Span::styled("    ", app.settings.default)]),
        Spans::from(vec![
            Span::styled(colour_to_string(app.settings.title_fg_colour),
            if app.edit_setting == EditSettingField::Title { app.settings.highlight } else { app.settings.default }),
            Span::styled("    ", app.settings.default)]),
    ])
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
                .style(default_style)
        );

    // Render example
    let example = Paragraph::new(vec![
        Spans::from(vec![Span::styled("[ ] This task is selected", app.settings.highlight)]),
        Spans::from(vec![Span::styled("[ ] This task is none of the above, just sitting here calmly", app.settings.default)]),
        Spans::from(vec![Span::styled("[ ] This task is none of the above, just sitting here calmly", app.settings.default)]),
        Spans::from(vec![Span::styled("[ ] This task is the active one", app.settings.active_normal)]),
        Spans::from(vec![Span::styled("[ ] This task is none of the above, just sitting here calmly", app.settings.default)]),
        Spans::from(vec![Span::styled("[X] This task is selected and active (although there can only be one active one", app.settings.active_highlight)]),
        Spans::from(vec![Span::styled("[ ] This task is none of the above, just sitting here calmly", app.settings.default)]),
    ])
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(default_style)
                .title(" Example ")
        );

    // Render instructions
    let instructions = Paragraph::new("enter - edit setting   | left/right - Modify    | 'h' - Go to Stats      | 'l' - Go to Tasks")
        .style(default_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(default_style)
                .border_type(BorderType::Double)
        );

    f.render_widget(tabs, chunks[0]);
    f.render_widget(settings_sections, hsplit_layout[0]);
    f.render_widget(settings_values, hsplit_layout[1]);
    f.render_widget(example, vsplit_layout[1]);
    f.render_widget(instructions, chunks[2]);
}