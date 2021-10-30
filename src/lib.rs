#![windows_subsystem = "windows"]
use crate::model::Computer;

mod ui;
mod model;
mod parse;

#[macro_export]
macro_rules! bit_at {
    ($opcode:expr, $pos:expr) => {
        {
            use core::ops::*;
            $opcode.bitand(1.shl($pos as u16) as u16) != 0
        }
    };
}

fn main() {
    println!("Hello, World!");
    let computer = Computer::new();


    ui::gui::Gui::new(computer).run();
}