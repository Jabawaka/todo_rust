// ----------------------------------------------------------------------------
// APP MODULE
// This module defines the apps behaviour. The file contains the interface of
// the app.
// ----------------------------------------------------------------------------

mod utils;
mod task;
mod renderer;

use utils::*;
use renderer::*;
use task::Task;

use std::io::Write;
use std::{fs, fs::File};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use std::path::Path;

use chrono::{Utc, DateTime};

use tui::style::Color;

use crossterm::event::{self, Event as CEvent, KeyCode};

use tui::{
    backend::Backend,
    style::Style,
    text::{Spans, Span},
    Terminal,
};

use serde::{Deserialize, Serialize};


// ---- CONSTANTS ----
const BLINK_TIME: Duration = Duration::from_millis(400);


enum Event<I> {
    Input(I),
    Tick,
}

#[derive(PartialEq)]
pub enum PopupType {
    NewTask,
    EditTask,
    ArchiveTasks,
}

#[derive(PartialEq, Copy, Clone)]
pub enum AppState {
    Display,
    EditTask,
    Archived,
    Settings,
}

impl From<AppState> for usize {
    fn from(input: AppState) -> usize {
        match input {
            AppState::Display     => 0,
            AppState::EditTask    => 0,
            AppState::Archived    => 1,
            AppState::Settings    => 2,
        }
    }
}

#[derive(PartialEq)]
enum EditSettingField {
    Split,
    NormalFg,
    NormalBg,
    SelectionFg,
    SelectionBg,
    Active,
    Title,
    Border,
}

#[derive(PartialEq)]
enum EditField {
    Title,
    Description,
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
    border: Style,

    // Colours for changing
    normal_fg_colour: Color,
    normal_bg_colour: Color,
    select_fg_colour: Color,
    select_bg_colour: Color,
    active_fg_colour: Color,
    title_fg_colour: Color,
    border_colour: Color,
}

impl Settings {
    fn set_colours(&mut self) {
        self.default = Style::default().fg(self.normal_fg_colour).bg(self.normal_bg_colour);
        self.highlight = Style::default().fg(self.select_fg_colour).bg(self.select_bg_colour);
        self.active_normal = Style::default().fg(self.active_fg_colour).bg(self.normal_bg_colour);
        self.active_highlight = Style::default().fg(self.active_fg_colour).bg(self.select_bg_colour);
        self.title = Style::default().fg(self.title_fg_colour).bg(self.normal_bg_colour);
        self.border = Style::default().fg(self.border_colour).bg(self.normal_bg_colour);
    }

    pub fn default_settings() -> Settings {
        let mut settings: Settings = Settings {
            is_horizontal: true,

            default:          Style::default(),
            highlight:        Style::default(),
            active_normal:    Style::default(),
            active_highlight: Style::default(),
            title:            Style::default(),
            border:           Style::default(),

            normal_fg_colour: Color::White,
            normal_bg_colour: Color::Black,
            select_fg_colour: Color::Black,
            select_bg_colour: Color::White,
            active_fg_colour: Color::Green,
            title_fg_colour:  Color::Green,
            border_colour:    Color::Green,
        };

        settings.set_colours();

        settings
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct ArchiveItem {
    date: DateTime<Utc>,
    tasks: Vec<Task>,
}

pub struct App {
    // App state
    data_path: String,
    last_event: Instant,
    tasks: Vec<Task>,
    archive: Vec<ArchiveItem>,
    curr_archive: usize,
    state: AppState,
    edit_field: EditField,
    edit_setting: EditSettingField,
    show_popup: bool,
    popup_type: PopupType,

    // Displaying variables
    desc_width_char: u16,
    task_block_height: u16,
    first_task: u16,

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
    pub fn new(path_to_folder: &String) -> Result<App, Box<dyn std::error::Error>> {
        let temp_path_to_db = Path::new(path_to_folder).join("tasks.json");
        let path_to_db = temp_path_to_db.as_path();
        let temp_path_to_archive = Path::new(&path_to_folder).join("archive.json");
        let path_to_archive = temp_path_to_archive.as_path();
        let temp_path_to_settings = Path::new(&path_to_folder).join("settings.json");
        let path_to_settings = temp_path_to_settings.as_path();

        if !path_to_db.exists() {
            let mut file = File::create(path_to_db)?;
            file.write_all(b"[]")?;
        }

        let db_content = fs::read_to_string(path_to_db)?;
        let mut parsed_tasks: Vec<Task> = serde_json::from_str(&db_content)?;

        for task in &mut parsed_tasks {
            task.is_selected = false;
        }

        if parsed_tasks.len() > 0 {
            parsed_tasks[0].is_selected = true;
        }

        if !path_to_archive.exists() {
            let mut file = File::create(path_to_archive)?;
            file.write_all(b"[]")?;
        }

        let archive_content = fs::read_to_string(path_to_archive)?;
        let archive_items: Vec<ArchiveItem> = serde_json::from_str(&archive_content)?;

        let settings: Settings;
        if path_to_settings.exists() {
            let settings_content = fs::read_to_string("settings.json")?;
            settings = serde_json::from_str(&settings_content)?;
        } else {
            settings = Settings::default_settings();
        }

        Ok(App {
            data_path: path_to_folder.clone(),
            last_event: Instant::now(),
            tasks: parsed_tasks.to_owned(),
            archive: if archive_items.len() > 0 {
                    archive_items.iter().map(|a| {
                    ArchiveItem {
                        date: a.date,
                        tasks: a.tasks.to_owned()}
                    }).collect()
                } else {
                    vec![]
                },
            curr_archive: if archive_items.len() > 0 {
                archive_items.len() - 1
            } else {
                0
            },
            state: AppState::Display,
            edit_field: EditField::Description,
            edit_setting: EditSettingField::Split,
            show_popup: false,
            popup_type: PopupType::NewTask,

            desc_width_char: 0,
            task_block_height: 0,
            first_task: 0,

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


    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), Box<dyn std::error::Error>> {
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
            terminal.draw(|f| term_ui(f, self))?;

            self.update_times();

            match self.state {
                AppState::Display => {
                    match rx.recv()? {
                        Event::Input(key) => {
                            match key.code {
                                KeyCode::Char('q') => {
                                    if self.show_popup && self.popup_type == PopupType::ArchiveTasks {
                                        self.show_popup = false;
                                    } else {
                                        self.save_to_db();
                                        self.save_settings();
                                        return Ok(())
                                    }
                                },
                                KeyCode::Esc => {
                                    if self.show_popup && self.popup_type == PopupType::ArchiveTasks {
                                        self.show_popup = false;
                                    } else {
                                        self.save_to_db();
                                        self.save_settings();
                                        return Ok(())
                                    }
                                },
                                KeyCode::Char('c') => {
                                    self.show_popup = true;
                                    self.popup_type = PopupType::ArchiveTasks;
                                },
                                KeyCode::Char('s') => self.save_to_db(),
                                KeyCode::Char('j') => self.inc_sel_task(),
                                KeyCode::Char('k') => self.dec_sel_task(),
                                KeyCode::Char('u') => self.move_task_down(),
                                KeyCode::Char('i') => self.move_task_up(),
                                KeyCode::Down => self.inc_sel_task(),
                                KeyCode::Up => self.dec_sel_task(),
                                KeyCode::Enter => {
                                    if self.show_popup && self.popup_type == PopupType::ArchiveTasks {
                                        self.archive_done_tasks();
                                        self.show_popup = false;
                                    } else {
                                        self.activate_task();
                                    }
                                },
                                KeyCode::Char(' ') => self.do_undo_task(),
                                KeyCode::Char('a') => self.add_task(),
                                KeyCode::Char('d') => self.del_task(),
                                KeyCode::Char('e') => {
                                    self.show_popup = true;
                                    self.popup_type = PopupType::EditTask;
                                    self.enter_edit(EditField::Description);
                                },
                                KeyCode::Tab => self.state = AppState::Archived,
                                KeyCode::BackTab => self.state = AppState::Settings,
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
                                KeyCode::Esc => self.enter_display(),
                                KeyCode::Backspace => self.delete_in_field(),
                                KeyCode::Enter => self.type_in_field('\n'),
                                KeyCode::Left => self.dec_cursor(),
                                KeyCode::Right => self.inc_cursor(),
                                KeyCode::Up => self.dec_line(),
                                KeyCode::Down => self.inc_line(),
                                KeyCode::Char(c) => self.type_in_field(c),
                                KeyCode::Tab => self.change_field(),
                                _ => {}
                            }
                        },
                        Event::Tick => {},
                    }
                },
                AppState::Archived => {
                    match rx.recv()? {
                        Event::Input(key) => {
                            match key.code {
                                KeyCode::Char('q') => {self.save_to_db(); self.save_settings(); return Ok(())},
                                KeyCode::Esc => {self.save_to_db(); self.save_settings(); return Ok(())},
                                KeyCode::Char('h') => self.inc_arch_item(),
                                KeyCode::Char('l') => self.dec_arch_item(),
                                KeyCode::Left => self.inc_arch_item(),
                                KeyCode::Right => self.dec_arch_item(),
                                KeyCode::Char('j') => self.inc_sel_task(),
                                KeyCode::Char('k') => self.dec_sel_task(),
                                KeyCode::Char(' ') => self.dearchive_task(),
                                KeyCode::Down => self.inc_sel_task(),
                                KeyCode::Up => self.dec_sel_task(),
                                KeyCode::Tab => self.state = AppState::Settings,
                                KeyCode::BackTab => self.enter_display(),
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
                                KeyCode::Char('q') => {self.save_to_db(); self.save_settings(); return Ok(())},
                                KeyCode::Esc => {self.save_to_db(); self.save_settings(); return Ok(())},
                                KeyCode::Char('h') => self.state = AppState::Display,
                                KeyCode::Char('l') => self.state = AppState::Display,
                                KeyCode::Up => self.dec_setting_selection(),
                                KeyCode::Down => self.inc_setting_selection(),
                                KeyCode::Right => self.inc_setting(),
                                KeyCode::Left => self.dec_setting(),
                                KeyCode::Tab => self.enter_display(),
                                KeyCode::BackTab => self.state = AppState::Archived,
                                _ => {}
                            }
                        },
                        Event::Tick => {},
                    }
                },
            }
        }
    }

    pub fn save_to_db(&mut self) {
        let mut full_path = self.data_path.clone();
        full_path.push_str("tasks.json");
        fs::write(full_path, &serde_json::to_vec_pretty(&self.tasks).expect("DB should be writeable")).expect("DB should be writeable");

        let mut arch_path = self.data_path.clone();
        arch_path.push_str("archive.json");
        fs::write(arch_path, &serde_json::to_vec_pretty(&self.archive).expect("Archive should be writeable")).expect("Archive should be writeable");
    }

    pub fn save_settings(&mut self) {
        let mut full_path = self.data_path.clone();
        full_path.push_str("settings.json");
        fs::write(full_path, &serde_json::to_vec_pretty(&self.settings).expect("Settings should be writeable")).expect("Settings should be writeable");
    }

    pub fn move_task_up(&mut self) {
        if self.tasks.len() > 1 {
            let mut index = self.tasks.len() - 1;
            while index > 0 {
                if self.tasks[index].is_selected {
                    let copy_task = self.tasks[index].clone();
                    self.tasks[index] = self.tasks[index - 1].clone();
                    self.tasks[index - 1] = copy_task;
                    break;
                }

                index -= 1;
            }
        }
    }

    pub fn move_task_down(&mut self) {
        if self.tasks.len() > 1 {
            let mut index = 0;
            while index < self.tasks.len() - 1 {
                if self.tasks[index].is_selected {
                    let copy_task = self.tasks[index].clone();
                    self.tasks[index] = self.tasks[index + 1].clone();
                    self.tasks[index + 1] = copy_task;
                    break;
                }

                index += 1;
            }
        }
    }

    pub fn inc_sel_task(&mut self) {
        let mut index = 0;

        match self.state {
            AppState::Display => {
                if self.tasks.len() > 0 {
                    while index < self.tasks.len() - 1 {
                        if self.tasks[index].is_selected {
                            self.tasks[index].is_selected = false;
                            self.tasks[index + 1].is_selected = true;

                            if (index + 1) as u16 >= self.first_task + self.task_block_height {
                                self.first_task = (index + 1) as u16 - (self.task_block_height - 1);
                            }
                            break;
                        }

                        index += 1;
                    }
                }
            },
            AppState::Archived => {
                if self.archive.len() > 0 {
                    if self.archive[self.curr_archive].tasks.len() > 0 {
                        while index < self.archive[self.curr_archive].tasks.len() - 1 {
                            if self.archive[self.curr_archive].tasks[index].is_selected {
                                self.archive[self.curr_archive].tasks[index].is_selected = false;
                                self.archive[self.curr_archive].tasks[index + 1].is_selected = true;

                                if (index + 1) as u16 >= self.first_task + self.task_block_height {
                                    self.first_task = (index + 1) as u16 - (self.task_block_height - 1);
                                }
                                break;
                            }

                            index += 1;
                        }
                    }
                }
            },
            _ => {}
        }
    }

    pub fn dec_sel_task(&mut self) {
        let mut index = 1;

        match self.state {
            AppState::Display => {
                while index < self.tasks.len() {
                    if self.tasks[index].is_selected {
                        self.tasks[index].is_selected = false;
                        self.tasks[index - 1].is_selected = true;

                        if ((index - 1) as u16) < self.first_task {
                            self.first_task = (index - 1) as u16;
                        }
                    }

                    index += 1;
                }
            },
            AppState::Archived => {
                if self.archive.len() > 0 {
                    while index < self.archive[self.curr_archive].tasks.len() {
                        if self.archive[self.curr_archive].tasks[index].is_selected {
                            self.archive[self.curr_archive].tasks[index].is_selected = false;
                            self.archive[self.curr_archive].tasks[index - 1].is_selected = true;

                            if ((index - 1) as u16) < self.first_task {
                                self.first_task = (index - 1) as u16;
                            }
                        }

                        index += 1;
                    }
                }
            },
            _ => {}
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
        let mut any_selected = false;
        self.show_popup = false;
        for task in &mut self.tasks {
            if task.is_selected {
                any_selected = true;

                if self.state == AppState::EditTask {
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
                }
            }
        }

        self.state = AppState::Display;

        if !any_selected && self.tasks.len() > 0 {
            self.tasks[0].is_selected = true;
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

    fn inc_arch_item(&mut self) {
        if self.archive.len() > 0 {
            if self.curr_archive < self.archive.len() - 1 {
                self.curr_archive += 1;
            }
        }
    }

    fn dec_arch_item(&mut self) {
        if self.curr_archive > 0 {
            self.curr_archive -= 1;
        }
    }

    fn archive_done_tasks(&mut self) {
        let mut new_arch_item = ArchiveItem {
            date: Utc::now(),
            tasks: vec![],
        };

        let mut index = 0;
        let mut reset_selection = false;
        while index < self.tasks.len() {
            if self.tasks[index].is_done {
                if self.tasks[index].is_selected {
                    self.tasks[index].is_selected = false;
                    reset_selection = true;
                }

                new_arch_item.tasks.push(self.tasks[index].clone());

                self.tasks.remove(index);
            } else {
                index += 1;
            }
        }

        if reset_selection && self.tasks.len() > 0 {
            self.tasks[0].is_selected = true;
        }

        if new_arch_item.tasks.len() > 0 {
            new_arch_item.tasks[0].is_selected = true;
            self.archive.push(new_arch_item.clone());
            self.curr_archive = self.archive.len() - 1;
        }
    }

    fn dearchive_task(&mut self) {
        if self.archive.len() > 0 {
            let mut index = 0;

            while index < self.archive[self.curr_archive].tasks.len() {
                if self.archive[self.curr_archive].tasks[index].is_selected {
                    self.archive[self.curr_archive].tasks[index].is_done = false;
                    self.archive[self.curr_archive].tasks[index].is_selected = false;
                    self.tasks.push(self.archive[self.curr_archive].tasks[index].clone());

                    self.archive[self.curr_archive].tasks.remove(index);

                    if self.archive[self.curr_archive].tasks.len() > 0 {
                        if index < self.archive[self.curr_archive].tasks.len() {
                            self.archive[self.curr_archive].tasks[index].is_selected = true;
                        } else {
                            self.archive[self.curr_archive].tasks[index - 1].is_selected = true;
                        }
                    }
                }

                index += 1;
            }

            if self.archive[self.curr_archive].tasks.len() == 0 {
                self.archive.remove(self.curr_archive);

                if self.archive.len() == 0 {
                    self.curr_archive = 0;
                } else if self.curr_archive >= self.archive.len() {
                    self.curr_archive = self.archive.len() - 1;
                }
            }
        }
    }

    fn get_curr_archive_item(&self) -> Option<ArchiveItem> {
        if self.archive.len() > 0 {
            let active_archive = self.archive[self.curr_archive].clone();
            return Some(active_archive);
        }
        None
    }

    fn get_sel_task_info(&mut self) -> Option<Vec<Spans>> {
        match self.state {
            AppState::Display => {
                for task in &self.tasks {
                    if task.is_selected {
                        let mut spans: Vec<Spans> = vec![];

                        self.disp_string = String::from("\n");
                        self.disp_string.push_str(&task.description);
                        let lines: Vec<&str> = self.disp_string.split("\n").collect();

                        for line in lines {
                            spans.push(Spans::from(vec![Span::styled(line, self.settings.default)]));
                        }

                        return Some(spans);
                    }
                }
            },
            AppState::EditTask => {
                for task in &self.tasks {
                    if task.is_selected {
                        let mut spans: Vec<Spans> = vec![];

                        self.disp_string = String::from("\n");
                        self.disp_string.push_str(&task.description);
                        let lines: Vec<&str> = self.disp_string.split("\n").collect();

                        for line in lines {
                            spans.push(Spans::from(vec![Span::styled(line, self.settings.default)]));
                        }

                        return Some(spans);
                    }
                }
            },
            AppState::Archived => {
                if self.archive.len() > 0 {
                    for task in &self.archive[self.curr_archive].tasks {
                        if task.is_selected {
                            let mut spans: Vec<Spans> = vec![];

                            self.disp_string = String::from("\n");
                            self.disp_string.push_str(&task.description);
                            let lines: Vec<&str> = self.disp_string.split("\n").collect();

                            for line in lines {
                                spans.push(Spans::from(vec![Span::styled(line, self.settings.default)]));
                            }

                            return Some(spans);
                        }
                    }
                }
            },
            _ => {}
        }

        None
    }

    fn get_sel_task_info_editable(&mut self) -> Option<Vec<Spans>> {
        match self.state {
            AppState::EditTask => {
                for task in &self.tasks {
                    if task.is_selected {
                        let mut spans: Vec<Spans> = vec![];
                        if self.edit_field == EditField::Description {
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
                                spans.push(Spans::from(vec![Span::styled(line, self.settings.default)]));
                            }
                        } else {
                            self.disp_string = String::from("\n");
                            self.disp_string.push_str(&task.description);
                            let lines: Vec<&str> = self.disp_string.split("\n").collect();

                            for line in lines {
                                spans.push(Spans::from(vec![Span::styled(line, self.settings.default)]));
                            }
                        }

                        return Some(spans);
                    }
                }
            },
            _ => {}
        }

        None
    }

    fn get_sel_task_title(&mut self) -> Option<String> {
        match self.state {
            AppState::Display => {
                for task in &self.tasks {
                    if task.is_selected {
                        return Some(task.title.clone());
                    }
                }
            },
            AppState::EditTask => {
                for task in &self.tasks {
                    if task.is_selected {
                        return Some(task.title.clone());
                    }
                }
            },
            AppState::Archived => {
                if self.archive.len() > 0 {
                    for task in &self.archive[self.curr_archive].tasks {
                        if task.is_selected {
                            return Some(task.title.clone());
                        }
                    }
                }
            },
            _ => {}
        }

        None
    }

    fn get_sel_task_title_editable(&mut self) -> Option<String> {
        match self.state {
            AppState::EditTask => {
                for task in &self.tasks {
                    if task.is_selected {
                        if self.edit_field == EditField::Title {
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
            },
            _ => {},
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

        self.show_popup = true;
        self.popup_type = PopupType::NewTask;

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
            EditSettingField::Title => self.edit_setting = EditSettingField::Border,
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
            EditSettingField::Border => self.edit_setting = EditSettingField::Title,
            _ => {},
        }
    }

    fn inc_setting(&mut self) {
        match self.edit_setting {
            EditSettingField::Split => self.settings.is_horizontal = !self.settings.is_horizontal,
            EditSettingField::NormalFg => {self.settings.normal_fg_colour = next_colour(self.settings.normal_fg_colour); self.settings.set_colours()},
            EditSettingField::NormalBg => {self.settings.normal_bg_colour = next_colour(self.settings.normal_bg_colour); self.settings.set_colours()},
            EditSettingField::SelectionFg => {self.settings.select_fg_colour = next_colour(self.settings.select_fg_colour); self.settings.set_colours()},
            EditSettingField::SelectionBg => {self.settings.select_bg_colour = next_colour(self.settings.select_bg_colour); self.settings.set_colours()},
            EditSettingField::Active => {self.settings.active_fg_colour = next_colour(self.settings.active_fg_colour); self.settings.set_colours()},
            EditSettingField::Title => {self.settings.title_fg_colour = next_colour(self.settings.title_fg_colour); self.settings.set_colours()},
            EditSettingField::Border => {self.settings.border_colour = next_colour(self.settings.border_colour); self.settings.set_colours()},
        }
    }

    fn dec_setting(&mut self) {
        match self.edit_setting {
            EditSettingField::Split => self.settings.is_horizontal = !self.settings.is_horizontal,
            EditSettingField::NormalFg => {self.settings.normal_fg_colour = prev_colour(self.settings.normal_fg_colour); self.settings.set_colours()},
            EditSettingField::NormalBg => {self.settings.normal_bg_colour = prev_colour(self.settings.normal_bg_colour); self.settings.set_colours()},
            EditSettingField::SelectionFg => {self.settings.select_fg_colour = prev_colour(self.settings.select_fg_colour); self.settings.set_colours()},
            EditSettingField::SelectionBg => {self.settings.select_bg_colour = prev_colour(self.settings.select_bg_colour); self.settings.set_colours()},
            EditSettingField::Active => {self.settings.active_fg_colour = prev_colour(self.settings.active_fg_colour); self.settings.set_colours()},
            EditSettingField::Title => {self.settings.title_fg_colour = prev_colour(self.settings.title_fg_colour); self.settings.set_colours()},
            EditSettingField::Border => {self.settings.border_colour = prev_colour(self.settings.border_colour); self.settings.set_colours()},
        }
    }
}