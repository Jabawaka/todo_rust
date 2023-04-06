use std::time::Duration;

use chrono::{DateTime, Utc};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Task {
    pub title: String,
    pub description: String,
    pub is_done: bool,
    pub is_active: bool,
    pub is_selected: bool,
    pub elapsed_time: Duration,
    pub created_on: DateTime<Utc>,
}

impl Task {
    pub fn get_time_str(&self) -> String {
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

    pub fn toggle_active(&mut self) {
        if self.is_active {
            self.is_active = false;
        } else {
            self.is_active = true;
        }
    }
}