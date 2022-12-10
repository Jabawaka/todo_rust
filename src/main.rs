use std::{error::Error, io, io::Write, env, fs, fs::File};
use std::time::{Duration, Instant};
use std::path::Path;

use chrono::prelude::*;

use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Spans, Span},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Tabs,
    },
    Frame, Terminal,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use serde::{Deserialize, Serialize};


// ---- STRUCT AND ENUM DEFINITION ----
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
    ellapsed_time: Duration,
    created_on: DateTime<Utc>,
}


struct App {
    tasks: Vec<Task>,
    state: AppState,
    selected_task: usize,
}

impl App {
    fn new(path_to_db: &String) -> Result<App, Box<dyn Error>> {
        let db_content = fs::read_to_string(path_to_db)?;
        let mut parsed_tasks: Vec<Task> = serde_json::from_str(&db_content)?;

        if parsed_tasks.len() > 0 {
            parsed_tasks[0].is_selected = true;
        }

        Ok(App {
            tasks: parsed_tasks.to_owned(),
            state: AppState::Display,
            selected_task: 0,
        })
    }

    fn inc_sel_task(&mut self) {
        let mut index = 0;

        while index < self.tasks.len() - 1 {
            if self.tasks[index].is_selected {
                self.tasks[index].is_selected = false;
                self.tasks[index + 1].is_selected = true;
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

    fn activate_task(&mut self) {
        let mut index = 0;
        while index < self.tasks.len() {
            self.tasks[index].is_active = self.tasks[index].is_selected;
            index += 1;
        }
    }
}


// ---- MAIN FUNCTION ----
fn main() -> Result<(), Box<dyn Error>> {
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

    let mut app = App::new(&path_to_db)?;

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
fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read().expect("Could not read events!") {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Down => app.inc_sel_task(),
                KeyCode::Up => app.dec_sel_task(),
                KeyCode::Enter => app.activate_task(),
                _ => {}
            }
        }
    }
}

// UI function
fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Min(2),
                Constraint::Length(3),
            ].as_ref(),
        ).split(size);

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

    let task_block = Paragraph::new(tasks)
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::ALL)
            .style(Style::default())
            .title(" To do ")
        );

    let instructions = Paragraph::new("Space - Mark task as done | A - Add task")
        .style(Style::default())
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default())
                .border_type(BorderType::Double)
        );

    f.render_widget(task_block, chunks[0]);
    f.render_widget(instructions, chunks[1]);
}