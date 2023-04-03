use crate::app::{App, Settings, AppState, EditSettingField, PopupType};
use crate::app::utils::*;

use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Spans, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph, Tabs, Wrap,
    },
    Frame,
};

pub fn term_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let chunks = create_chunks(f);
    render_menu(f, &chunks[0], app);

    let disp_instructions = "' ' - Mark task as done | 'a' - Add task         | 'e' - Edit task        | 'd' - Delete task      \n'j' - Go up             | 'k' - Go down          | Tab - Archive          | Shift+Tab - Settings  \n'c' - Archive tasks     | 's' - Save tasks       | enter - Activate task  | esc,'q' - Quit         ";
    let arch_instructions = "'j' - Go up             | 'k' - Go down          | Tab - Settings         | Shift+Tab - Tasks      \n'h' - Newer archive     | 'l' - Older archive    | ' ' - Dearchive task   | esc,'q' - Quit        ";
    let sett_instructions = "Up/Down - Select        | Left/Right - Modify    | Tab - Archive          | Shift+Tab - Tasks      ";

    match app.state {
        AppState::Display  => {
            render_tasks(f, &chunks[1], app);
            render_instructions(f, &chunks[2], &app.settings, &disp_instructions);
        },
        AppState::EditTask => {
            render_tasks(f, &chunks[1], app);
            render_instructions(f, &chunks[2], &app.settings, &disp_instructions);
        },
        AppState::Archived => {
            render_archived(f, &chunks[1], app);
            render_instructions(f, &chunks[2], &app.settings, &arch_instructions);
        },
        AppState::Settings => {
            render_settings(f, &chunks[1], app);
            render_instructions(f, &chunks[2], &app.settings, &sett_instructions);
        },
    }
}


// Create main layout chunks
fn create_chunks<B: Backend>(f: &mut Frame<B>) -> Vec<Rect> {
    let size = f.size();

    Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(2),
                Constraint::Min(2),
                Constraint::Length(4),
            ].as_ref(),
        ).split(size)
}


// Pop up layout
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}


// Render menu
fn render_menu<B: Backend>(f: &mut Frame<B>, rect: &Rect, app: &App) {
    let menu_titles = vec!["Active tasks", "Archived tasks", "Settings"];
    let menu = menu_titles
        .iter()
        .map(|t| {
            let (first, rest) = t.split_at(1);
            Spans::from(vec![
                Span::styled(
                    first,
                    app.settings.title,
                ),
                Span::styled(rest, app.settings.default),
            ])
        })
        .collect();

    let state = app.state.into();
    let tabs = Tabs::new(menu)
        .select(state)
        .block(Block::default().borders(Borders::BOTTOM).border_type(BorderType::Double).style(app.settings.border))
        .style(app.settings.default)
        .highlight_style(app.settings.title)
        .divider(Span::styled("||", app.settings.default));

    f.render_widget(tabs, *rect);
}


// Render instructions
fn render_instructions<B: Backend>(f: &mut Frame<B>, rect: &Rect, settings: &Settings, inst_str: &str) {
    // Render instructions
    let instructions = Paragraph::new(inst_str)
        .style(settings.border)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(settings.border)
                .border_type(BorderType::Double)
        );

    f.render_widget(instructions, *rect);
}


// Render tasks screen
fn render_tasks<B: Backend>(f: &mut Frame<B>, rect: &Rect, app: &mut App) {
    let vsplit_layout = if app.settings.is_horizontal {
        Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(*rect)
    } else {
        Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(*rect)
    };

    let hsplit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(4),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]
        ).split(vsplit_layout[0]);

    // Capture displaying variables
    app.desc_width_char = vsplit_layout[1].width - 2;
    app.task_block_height = hsplit_layout[0].height - 2;
    let default_style = app.settings.default.clone();
    let border_style = app.settings.border.clone();
    let title_style = app.settings.title.clone();

    // Render scroll bar
    let mut line = 0;
    let mut scroll_bar = vec![];
    let scroll_perc;
    if app.tasks.len() > app.task_block_height as usize {
        scroll_perc = (app.first_task as f32) / ((app.tasks.len() as u16 - app.task_block_height) as f32);
        let scroll_line = (scroll_perc * (app.task_block_height - 1) as f32) as u16;
        let scroll_size = (((app.task_block_height as f32) * (app.task_block_height as f32) / (app.tasks.len() as f32)).floor()) as u16;

        while line < app.task_block_height {
            if line >= scroll_line && line <= (scroll_line + scroll_size) {
                scroll_bar.push(Spans::from(vec![Span::styled("█", app.settings.border)]));
            } else {
                scroll_bar.push(Spans::from(vec![Span::styled("│", app.settings.border)]));
            }
            line += 1;
        }
    } else {
        while line < app.task_block_height {
            scroll_bar.push(Spans::from(vec![Span::styled(" ", app.settings.border)]));
            line += 1;
        }
    }

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

    if tasks.len() > app.task_block_height as usize {
        let first_index = app.first_task as usize;
        let last_index = (app.first_task + app.task_block_height) as usize;
        tasks = tasks[first_index..last_index].to_vec();
    }

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

    if tasks_duration.len() > app.task_block_height as usize {
        let first_index = app.first_task as usize;
        let last_index = (app.first_task + app.task_block_height) as usize;
        tasks_duration = tasks_duration[first_index..last_index].to_vec();
    }

    let scroll_block = Paragraph::new(scroll_bar)
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .style(border_style)
        );

    let task_block = Paragraph::new(tasks)
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .style(border_style)
            .title(" To Do ")
        );

    let task_dur_block = Paragraph::new(tasks_duration)
        .alignment(Alignment::Right)
        .block(
            Block::default()
            .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
            .style(border_style)
        );

    let mut task_title = String::from("");
    if let Some(title) = app.get_sel_task_title() {
        task_title = title;
        task_title.insert(0, ' ');
        task_title.push(' ');
    }

    let task_description = Paragraph::new(app.get_sel_task_info().unwrap_or_else(|| { vec![Spans::from(vec![Span::styled("", default_style)])] }))
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::ALL)
            .style(border_style)
            .title(task_title)
        )
        .wrap(Wrap { trim: false });

    f.render_widget(scroll_block, hsplit_layout[0]);
    f.render_widget(task_block, hsplit_layout[1]);
    f.render_widget(task_dur_block, hsplit_layout[2]);
    f.render_widget(task_description, vsplit_layout[1]);

    // Show whatever popup is needed
    if app.show_popup {
        let area: Rect;
        let title: String;
        let alignment: Alignment;
        let mut popup_content: Vec<Spans>;

        match app.popup_type {
            PopupType::NewTask => {
                area = centered_rect(60, 60, f.size());
                title = String::from("New task");
                alignment = Alignment::Left;

                let mut edit_task_title = String::from("");
                if let Some(title) = app.get_sel_task_title_editable() {
                    edit_task_title = title;
                }
                let mut edit_task_desc = app.get_sel_task_info_editable().unwrap_or_else(|| { vec![Spans::from(vec![Span::styled("", default_style)])]});

                popup_content = vec![
                    Spans::from(vec![Span::styled("", default_style)]),
                    Spans::from(vec![Span::styled("Title:", title_style)]),
                    Spans::from(vec![Span::styled("", default_style)]),
                    Spans::from(vec![Span::styled(edit_task_title, default_style)]),
                    Spans::from(vec![Span::styled("", default_style)]),
                    Spans::from(vec![Span::styled("Description:", title_style)])
                    ];
                popup_content.append(&mut edit_task_desc);
            },
            PopupType::EditTask => {
                area = centered_rect(60, 60, f.size());
                title = String::from("Edit task");
                alignment = Alignment::Left;

                let mut edit_task_title = String::from("");
                if let Some(title) = app.get_sel_task_title_editable() {
                    edit_task_title = title;
                }
                let mut edit_task_desc = app.get_sel_task_info_editable().unwrap_or_else(|| { vec![Spans::from(vec![Span::styled("", default_style)])]});

                popup_content = vec![
                    Spans::from(vec![Span::styled("", default_style)]),
                    Spans::from(vec![Span::styled("Title:", title_style)]),
                    Spans::from(vec![Span::styled("", default_style)]),
                    Spans::from(vec![Span::styled(edit_task_title, default_style)]),
                    Spans::from(vec![Span::styled("", default_style)]),
                    Spans::from(vec![Span::styled("Description:", title_style)])
                    ];
                popup_content.append(&mut edit_task_desc);
            },
            PopupType::ArchiveTasks => {
                area = centered_rect(25, 25, f.size());
                title = String::from("Confirm archiving");
                alignment = Alignment::Center;

                popup_content = vec![
                    Spans::from(vec![Span::styled("", default_style)]),
                    Spans::from(vec![Span::styled("Do you want to archive done tasks?", title_style)]),
                    Spans::from(vec![Span::styled("", default_style)]),
                    Spans::from(vec![Span::styled("Press enter to confirm, esc to cancel", default_style)])
                ];
            },
        }

        let edit_box = Paragraph::new(popup_content)
            .alignment(alignment)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(border_style)
                    .title(title)
            )
            .wrap(Wrap { trim: false});

        f.render_widget(Clear, area);
        f.render_widget(edit_box, area);
    }
}


// Render archived screen
fn render_archived<B: Backend>(f: &mut Frame<B>, rect: &Rect, app: &mut App) {
    let vsplit_layout = if app.settings.is_horizontal {
        Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(*rect)
    } else {
        Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(*rect)
    };

    let hsplit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(4),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]
        ).split(vsplit_layout[0]);

    // Capture displaying variables
    app.desc_width_char = vsplit_layout[1].width - 2;
    let default_style = app.settings.default.clone();
    let border_style = app.settings.border.clone();

    // Render archive items
    let mut archive_title = String::from("");
    let mut archive_tasks: Vec<_> = vec![];
    let mut archive_durations: Vec<_> = vec![];
    if let Some(archive_item) = app.get_curr_archive_item() {
        let converted_date = format!("{}", archive_item.date.format("%Y/%m/%d"));
        archive_title.push_str(&converted_date);
        if app.archive.len() > 0 {
            if app.curr_archive > 0 {
                archive_title.push(' ');
                archive_title.push('-');
                archive_title.push('>');
            } else {
                archive_title.push(' ');
                archive_title.push(' ');
                archive_title.push(' ');
            }

            if app.curr_archive < app.archive.len() - 1 {
                archive_title.insert(0, ' ');
                archive_title.insert(0, '-');
                archive_title.insert(0, '<');
            } else {
                archive_title.insert(0, ' ');
                archive_title.insert(0, ' ');
                archive_title.insert(0, ' ');
            }
        }

        archive_title.insert(0, ' ');
        archive_title.push(' ');

        archive_tasks = archive_item.tasks
            .iter()
            .map(|task| {
                let mut disp_string = String::from("[X] ");
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

        archive_durations = archive_item.tasks
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
    }

    // Render scroll bar
    let mut scroll_bar = vec![];
    if app.archive.len() > 0 {
        let mut line = 0;
        let scroll_perc;
        if app.archive[app.curr_archive].tasks.len() > app.task_block_height as usize {
            scroll_perc = (app.first_task as f32) / ((app.archive[app.curr_archive].tasks.len() as u16 - app.task_block_height) as f32);
            let scroll_line = (scroll_perc * (app.task_block_height - 1) as f32) as u16;

            while line < app.task_block_height {
                if line == scroll_line {
                    scroll_bar.push(Spans::from(vec![Span::styled("█", app.settings.border)]));
                } else {
                    scroll_bar.push(Spans::from(vec![Span::styled("│", app.settings.border)]));
                }
                line += 1;
            }
        } else {
            while line < app.task_block_height {
                scroll_bar.push(Spans::from(vec![Span::styled(" ", app.settings.border)]));
                line += 1;
            }
        }


        if app.archive[app.curr_archive].tasks.len() > app.task_block_height as usize {
            let first_index = app.first_task as usize;
            let last_index = (app.first_task + app.task_block_height) as usize;
            archive_tasks = archive_tasks[first_index..last_index].to_vec();
            archive_durations = archive_durations[first_index..last_index].to_vec();
        }
    }

    let archive_block = Paragraph::new(archive_tasks)
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .style(border_style)
            .title(archive_title)
        );

    let archive_dur_block = Paragraph::new(archive_durations)
        .alignment(Alignment::Right)
        .block(
            Block::default()
            .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
            .style(border_style)
        );

    let scroll_block = Paragraph::new(scroll_bar)
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .style(border_style)
        );

    let mut task_title = String::from(" ");
    task_title.push_str(&app.get_sel_task_title().unwrap_or_else(|| { String::from("") }));
    task_title.push_str(" ");

    let task_description = Paragraph::new(app.get_sel_task_info().unwrap_or_else(|| { vec![Spans::from(vec![Span::styled("", default_style)])] }))
        .alignment(Alignment::Left)
        .block(
            Block::default()
            .borders(Borders::ALL)
            .style(border_style)
            .title(task_title)
        )
        .wrap(Wrap { trim: false });

    f.render_widget(scroll_block, hsplit_layout[0]);
    f.render_widget(archive_block, hsplit_layout[1]);
    f.render_widget(archive_dur_block, hsplit_layout[2]);
    f.render_widget(task_description, vsplit_layout[1]);
}


// Render settings
fn render_settings<B: Backend>(f: &mut Frame<B>, rect: &Rect, app: &mut App) {
    let vsplit_layout = if app.settings.is_horizontal {
        Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(*rect)
    } else {
        Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        ).split(*rect)
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
    let border_style = app.settings.border.clone();

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
        Spans::from(vec![
            Span::styled("  ", app.settings.default),
            Span::styled(
                "Border colour",
                if app.edit_setting == EditSettingField::Border { app.settings.highlight } else { app.settings.default }
            )]),
    ])
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
                .border_style(border_style)
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
        Spans::from(vec![
            Span::styled(colour_to_string(app.settings.border_colour),
            if app.edit_setting == EditSettingField::Border { app.settings.highlight } else { app.settings.default }),
            Span::styled("    ", app.settings.default)]),
    ])
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
                .style(border_style)
        );

    // Render example
    let example = Paragraph::new(vec![
        Spans::from(vec![Span::styled("", app.settings.default)]),
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
                .style(border_style)
                .title(" Example ")
        );

    f.render_widget(settings_sections, hsplit_layout[0]);
    f.render_widget(settings_values, hsplit_layout[1]);
    f.render_widget(example, vsplit_layout[1]);
}
