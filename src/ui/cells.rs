use crate::model::{Computer, MemoryCell, Memory, Register};
use imgui::{Ui, ChildWindow, StyleColor, Id, im_str, ImString, MenuItem, FocusedWidget, StyleVar, Io};
use crate::parse::mc::parse;
use crate::parse::{Parser, CommandInfo};
use crate::ui::window::Tool;
use crate::ui::gui::{PopupManager, Gui, GuiState};
use std::rc::Rc;
use std::cell::RefCell;
use imgui::__core::cell::RefMut;
use crate::ui::popup::{PopupParseError, PopupMessage};
use std::fs::{OpenOptions, File};
use std::io::{Write, BufReader, BufRead};


#[derive(PartialEq, Eq)]
enum CellRepresentation {
    Hex,
    Binary,
}

impl CellRepresentation {
    fn title(&self) -> String {
        return match self {
            CellRepresentation::Hex => "Шестнадцетеричное".to_string(),
            CellRepresentation::Binary => "Бинарное".to_string(),
        };
    }

    fn draw_hex(&self, cell: &mut MemoryCell, ui: &Ui) {
        let mut cell = cell;
        let mut data = ImString::from(format!("{:0>4X}", cell.get()));
        let width_t = ui.push_item_width(70.0);
        if ui.input_text(im_str!(""), &mut data)
            .chars_hexadecimal(true)
            .chars_noblank(true)
            .build() {
            let data = data.to_str();
            if let Ok(parsed) = u16::from_str_radix(data, 16) {
                cell.set(parsed)
            }
        }
        width_t.pop(ui);
    }
    fn draw_binary(&self, cell: &mut MemoryCell, ui: &Ui) {
        let mut cell = cell;
        let mut data = ImString::from(format!("{:0>16b}", cell.get()));
        let width_t = ui.push_item_width(160.0);
        if ui.input_text(im_str!(""), &mut data)
            .chars_decimal(true)
            .chars_noblank(true)
            .build() {
            let data = data.to_str();
            if let Ok(parsed) = u16::from_str_radix(data, 2) {
                cell.set(parsed)
            }
        }
        width_t.pop(ui);
    }
    fn draw(&self, cell: &mut MemoryCell, ui: &Ui) {
        match self {
            CellRepresentation::Hex => self.draw_hex(cell, ui),
            CellRepresentation::Binary => self.draw_binary(cell, ui),
        }
    }
}

pub struct CellsTool<I: CommandInfo, P: Parser<I>, F>
    where F: Fn(&Computer) -> u16
{
    page: Rc<RefCell<Memory<I, P>>>,
    counter_register: F,
    representation: CellRepresentation,
}

impl<I: CommandInfo, P: Parser<I>, F: Fn(&Computer) -> u16> Tool for CellsTool<I, P, F>
    where I: 'static
{
    fn draw(&mut self, ui: &Ui, io: &Io, state: &mut GuiState) {
        let mut idx = 0u32;

        self.draw_menu_bar(state, ui);

        let jump_needed = ui.button(im_str!("Перейти к исполняемой команде"), [0.0, 0.0]);

        let w_token = ChildWindow::new("cells_inside")
            .always_vertical_scrollbar(true)
            .border(true)
            .begin(ui);

        if w_token.is_none() {
            return;
        }

        let w_token = w_token.unwrap();


        let current_executed = (self.counter_register)(&mut state.computer);

        let mut next_rev_focused = false;

        let (parser, mut data) = RefMut::map_split(self.page.borrow_mut(), |r| (&mut r.parser, &mut r.data));

        let mut focused: Option<I> = None;

        for cell in data.iter_mut() {
            let token = ui.push_id(Id::Int(idx as i32));
            ui.text(format!("{:0>3X}", idx));
            ui.same_line(0.0);
            let t = if current_executed == idx as u16 {
                if jump_needed {
                    ui.set_scroll_here_y();
                }
                Some(ui.push_style_color(StyleColor::FrameBg, [1.0, 0.0, 0.0, 1.0]))
            } else {
                None
            };
            self.representation.draw(cell, ui);
            if let Some(t) = t {
                t.pop(ui);
            }
            if ui.is_item_focused() {
                focused = Some(parser.parse(cell.get()))
            }


            ui.same_line(0.0);
            let command = parser.parse(cell.get());

            if parser.supports_rev_parse() {
                let mut content = ImString::with_capacity(50);
                if next_rev_focused {
                    ui.set_keyboard_focus_here(FocusedWidget::Next);
                    next_rev_focused = false
                }
                content.push_str(command.mnemonic().as_str());
                if ui.input_text(im_str!("###mnemonic"), &mut content)
                    .callback_always(false)
                    .enter_returns_true(true)
                    .build()
                {
                    match parser.rev_parse(content.to_str()) {
                        Ok(opcode) => {
                            next_rev_focused = true;
                            cell.set(opcode);
                        }
                        Err(msg) => {
                            state.popup_manager.open(PopupParseError::new(content.to_string(), msg.to_string()))
                        }
                    }
                }

                if ui.is_item_focused() {
                    focused = Some(command)
                }
            } else {
                ui.text(command.mnemonic().as_str());
            }

            token.pop(ui);
            idx += 1;
        }


        if focused.is_some() {
            state.current_command = Some(Box::new(focused.unwrap()));
        } else {
            state.current_command = Some(Box::new(parser.parse(data.get(current_executed as usize).unwrap().get())));
        }

        w_token.end(ui);
    }
}

impl<I: CommandInfo, P: Parser<I>, F: Fn(&Computer) -> u16> CellsTool<I, P, F> {
    pub fn new(page: Rc<RefCell<Memory<I, P>>>, counter_register: F) -> CellsTool<I, P, F> {
        CellsTool {
            counter_register,
            page,
            representation: CellRepresentation::Hex,
        }
    }

    fn draw_menu_bar(&mut self, state: &mut GuiState, ui: &Ui) {
        ui.menu_bar(|| {
            ui.menu(im_str!("Опции"), true, || {
                self.draw_file_actions(state, ui);
                self.draw_representation_selection(ui);
            });
        })
    }


    fn on_save_to_file(&mut self, state: &mut GuiState) {
        let filename = match nfd::open_pick_folder(None) {
            Ok(r) => match r {
                nfd::Response::Okay(f) => {
                    f
                }
                _ => {return;}
            }
            Err(e) => {
                state.popup_manager.open(PopupMessage::new("Ошибка выбора папки", format!("Не могу открыть окно выбора папки: {}", e.to_string())));
                return;
            }
        };

        let filename = format!("{}/{}.mm", filename, self.page.borrow().name);

        match self.save_to_file(filename.as_str()) {
            Ok(_) => state.popup_manager.open(PopupMessage::new("Успех", format!("Успешно сохранил в файл {}", filename))),
            Err(e) => state.popup_manager.open(PopupMessage::new("Провал", format!("Не могу сохранить в файл \"{}\": {}", filename, e)))
        }
    }

    fn save_to_file(&mut self, file: &str) -> Result<(), String> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(false)
            .write(true)
            .truncate(true)
            .open(file)
            .map_err(|e| e.to_string())?;


        let mut s = String::new();
        let mut prev_zero = true;
        let mut prev_prev_zero = true;

        let mut pos = 0usize;
        for cell in &self.page.borrow().data {
            prev_prev_zero = prev_zero;

            let v = cell.get();
            if v == 0 {
                prev_zero = true
            } else {
                if prev_prev_zero && prev_zero {
                    s.push_str(format!("$pos {:X}\n", pos).as_str())
                }
                let str = self.page.borrow().parser.parse(v).file_string();
                s.push_str(str.as_str());
                s.push('\n');
                prev_zero = false;
            }

            pos += 1;
        }

        f.write(s.as_bytes());
        f.flush();

        Ok(())
    }

    fn choose_file(state: &mut GuiState, filter: Option<&str>) -> Option<File>{
        let file_name = match nfd::open_file_dialog(filter, None) {
            Ok(r) => match r {
                nfd::Response::Okay(f) => {
                    f
                }
                _ => { return None; }
            }
            Err(e) => {
                state.popup_manager.open(PopupMessage::new("Ошибка выбора файла", format!("Не могу открыть окно выбора файла: {}", e.to_string())));
                return None;
            }
        };


        let mut f = match File::open(file_name) {
            Ok(mut f) => f,
            Err(e) => {
                state.popup_manager.open(PopupMessage::new("Ошибка открытия файла", e.to_string()));
                return None;
            }
        };

        Some(f)
    }

    fn on_load_from_file(&mut self, state: &mut GuiState) {
        let f = Self::choose_file(state, Some("mm"));
        if f.is_none() { return; }

        let mut f = f.unwrap();


        let parse_result = crate::parse::file::parse_file(&mut f, &self.page.borrow().parser, 0xFF);


        if parse_result.is_err() {
            let msg = parse_result.unwrap_err();
            state.popup_manager.open(PopupMessage::new("Ошибка во время парсинга", msg));
            return;
        }

        let parse_result = parse_result.unwrap();

        let mem = &mut self.page.borrow_mut().data;
        for x in mem.iter_mut() {
            x.set(0)
        }

        for (pos, v) in parse_result {
            mem.get_mut(pos as usize).unwrap().set(v);
        }
    }

    fn load_bpc(&mut self, state: &mut GuiState) {
        let f = Self::choose_file(state, Some("bpc"));
        if f.is_none() { return; }

        let mut f = f.unwrap();

        let mut start_pos = 0;

        let mut parse_result = Vec::<(u16, u16)>::new();

        let mut line_num = 0;
        for line in BufReader::new(f).lines() {
            line_num+=1;
            match line {
                Ok(line) => {
                    let split: Vec<&str> = line.split(" ").collect();

                    if split.len() < 2 {
                        state.popup_manager.open(PopupMessage::new("Ошибочка", format!("Неверный формат({}) на строчке {}", line, line_num)));
                        return;
                    }

                    let pos = u16::from_str_radix(split.get(0).unwrap(),16);
                    if let Err(e) = pos {
                        state.popup_manager.open(PopupMessage::new("Ошибочка", format!("Не могу распарсить позицию {} на строчке {}", split[0], line_num)));
                        return;
                    }
                    let pos = pos.unwrap();

                    let mut cmd_str = split.get(0).unwrap().clone();
                    if cmd_str.len() > 0 && cmd_str.chars().nth(0).unwrap() == '+' {
                        start_pos = pos;
                        cmd_str = &cmd_str[1..];
                    }

                    let cmd = u16::from_str_radix(cmd_str, 16);
                    if let Err(_) = cmd {
                        state.popup_manager.open(PopupMessage::new("Ошибочка", format!("Не могу распарсить позицию {} на строчке {}", cmd_str, line_num)));
                        return;
                    }
                    let cmd = cmd.unwrap();

                    parse_result.push((pos, cmd))
                },
                Err(e) => state.popup_manager.open(PopupMessage::new("Ошибочка", e.to_string()))
            }
        }

        let mem = &mut self.page.borrow_mut().data;
        for x in mem.iter_mut() {
            x.set(0)
        }

        for (pos, v) in parse_result {
            mem.get_mut(pos as usize).unwrap().set(v);
        }

    }

    fn draw_file_actions(&mut self, state: &mut GuiState, ui: &Ui) {
        if let Some(token) = ui.begin_menu(im_str!("Файл"), true) {
            if MenuItem::new(im_str!("Сохранить")).build(ui) {
                self.on_save_to_file(state);
            }
            if MenuItem::new(im_str!("Загрузить")).build(ui) {
                self.on_load_from_file(state);
            }
            if MenuItem::new(im_str!("Загрузить .bpc")).build(ui) {
                self.load_bpc(state);
            }

            token.end(ui)
        }
    }


    fn draw_representation_selection(&mut self, ui: &Ui) {
        if let Some(token) = ui.begin_menu(im_str!("Представление ячеек"), true) {
            if MenuItem::new(ImString::from(CellRepresentation::Hex.title()).as_ref())
                .selected(self.representation == CellRepresentation::Hex)
                .build(ui)
            {
                self.representation = CellRepresentation::Hex
            }
            if MenuItem::new(ImString::from(CellRepresentation::Binary.title()).as_ref())
                .selected(self.representation == CellRepresentation::Binary)
                .build(ui)
            {
                self.representation = CellRepresentation::Binary
            }
            token.end(ui)
        }
    }

    pub fn draw(&mut self, computer: &mut Computer, ui: &Ui) {}
}