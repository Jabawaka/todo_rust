// ----------------------------------------------------------------------------
// UTILITIES SUB-MODULE
// This submodule defines a series of utility functions to be used within the
// App module.
// ----------------------------------------------------------------------------

use tui::style::Color;

// This function takes a Color from the TUI crate and returns the corresponding string to be shown.
pub fn colour_to_string(colour: Color) -> String {
    match colour {
        Color::White    => String::from("White"),
        Color::Cyan     => String::from("Cyan"),
        Color::Red      => String::from("Red"),
        Color::Green    => String::from("Green"),
        Color::Blue     => String::from("Blue"),
        Color::Yellow   => String::from("Yellow"),
        Color::Gray     => String::from("Gray"),
        Color::DarkGray => String::from("Dark gray"),
        Color::Black    => String::from("Black"),
        _               => String::from("Unknown"),
    }
}

pub fn next_colour(colour: Color) -> Color {
    match colour {
        Color::White    => Color::Cyan,
        Color::Cyan     => Color::Red,
        Color::Red      => Color::Green,
        Color::Green    => Color::Blue,
        Color::Blue     => Color::Yellow,
        Color::Yellow   => Color::Gray,
        Color::Gray     => Color::DarkGray,
        Color::DarkGray => Color::Black,
        Color::Black    => Color::White,
        _               => Color::Reset,
    }
}

pub fn prev_colour(colour: Color) -> Color {
    match colour {
        Color::White => Color::Black,
        Color::Cyan => Color::White,
        Color::Red => Color::Cyan,
        Color::Green => Color::Red,
        Color::Blue => Color::Green,
        Color::Yellow => Color::Blue,
        Color::Gray => Color::Yellow,
        Color::DarkGray => Color::Gray,
        Color::Black => Color::DarkGray,
        _ => Color::Reset,
    }
}