use core;
use midi;
use std::io::{stdin, Stdout, Write};

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{RawTerminal};
use termion::{clear, color, cursor, style};
use std::error::Error;

#[derive(Debug)]
pub struct ParamMenu<T> {
    device: Box<core::Device<PARAM = T>>,
    current_param: T,
}

const PARAM_START: u16 = 2;
const PARAM_LEN: u16 = 20;
const PARAM_VALUE_SPLIT: u16 = 2;
const VALUE_START: u16 = PARAM_START + PARAM_LEN + PARAM_VALUE_SPLIT;

impl<T: core::ParamEnum> ParamMenu<T> {
    pub fn new(device: Box<core::Device<PARAM = T>>, selected: Option<T>) -> Self {
        ParamMenu {
            device,
            current_param: selected.unwrap_or(T::from_ordinal(0).unwrap()),
        }
    }

    pub fn run(&mut self, stdout: &mut RawTerminal<Stdout>) -> core::Result<()> {
        self.print_all(stdout);
        let input = stdin();
        for c in input.keys() {
            match c.unwrap() {
                Key::Esc => break,
                // TODO define generic shortcuts
                Key::Left | Key::Char('-') => self.decrease(stdout),
                Key::Right | Key::Char('+') => self.increase(stdout),
                Key::Up | Key::Char('\t') => self.select_prev(stdout),
                Key::Down => self.select_next(stdout),
                _ => {}
            }
        }
        Ok(())
    }

    fn selected_value(&self, p: T) -> midi::MidiValue {
        self.device.get(p).expect("Mapped Parameter")
    }

    fn print_all(&mut self, stdout: &mut RawTerminal<Stdout>) {
        write!(stdout, "{}", clear::All).unwrap();
        for p in self.device.parameters().keys() {
            self.print_param(*p, stdout);
        }
    }

    fn print_value(&self, p: T, stdout: &mut RawTerminal<Stdout>) {
        let row = p.ordinal() as u16 + 2;
        let pdef = self.device.parameters().get(&p).expect("Parameter Def");
        let pvalue = pdef
            .value_name(self.selected_value(p))
            .unwrap_or("#INVALID#".into());
        if p == self.current_param {
            write!(stdout, "{bg}", bg = color::Bg(color::AnsiValue(237))).unwrap();
        };
        write!(
            stdout,
            "{goto}{fg}{pvalue: <20}{reset}",
            goto = cursor::Goto(VALUE_START, row),
            fg = color::Fg(color::Yellow),
            pvalue = pvalue,
            reset = style::Reset,
        ).unwrap();
        stdout.flush().unwrap()
    }

    fn print_param(&self, p: T, stdout: &mut RawTerminal<Stdout>) {
        let row = p.ordinal() as u16 + 2;
        let pdef = self.device.parameters().get(&p).expect("Parameter Def");
        if p == self.current_param {
            write!(stdout, "{bg}", bg = color::Bg(color::LightWhite)).unwrap();
        };
        write!(
            stdout,
            "{goto}{fg}{pname: <20}  {reset}",
            goto = cursor::Goto(PARAM_START, row),
            fg = color::Fg(color::Green),
            pname = pdef.name(),
            reset = style::Reset,
        ).unwrap();
        self.print_value(p, stdout);
        stdout.flush().unwrap()
    }

    fn increase(&mut self, stdout: &mut RawTerminal<Stdout>) {
        let mut value = self.selected_value(self.current_param);
        let pdef = self.device.parameters().get(&self.current_param).unwrap();
        let max = pdef.values().last().unwrap() - 1;
        if value < max {
            value += 1;
            if let Err(err) = self.device.set(self.current_param, value) {
                return self.print_error(err, stdout)
            }
            self.print_value(self.current_param, stdout);
        }
    }

    fn decrease(&mut self, stdout: &mut RawTerminal<Stdout>) {
        let mut value = self.selected_value(self.current_param);
        let pdef = self.device.parameters().get(&self.current_param).unwrap();
        let min = *pdef.values().first().unwrap();
        if value > min {
            value -= 1;
            if let Err(err) = self.device.set(self.current_param, value) {
                return self.print_error(err, stdout)
            }
            self.print_value(self.current_param, stdout);
        }
    }

    fn print_error(&mut self, err: Box<Error>, stdout: &mut RawTerminal<Stdout>) {
        write!(
            stdout,
            "{goto}{fg}{err}{reset}",
            goto = cursor::Goto(0, 0),
            err = err,
            fg = color::Fg(color::Red),
            reset = style::Reset,
        ).unwrap()
    }

    fn select(&mut self, p: T, stdout: &mut RawTerminal<Stdout>) {
        let prev = self.current_param;
        self.current_param = p;
        self.print_param(prev, stdout);
        self.print_param(self.current_param, stdout)
    }

    fn select_next(&mut self, stdout: &mut RawTerminal<Stdout>) {
        let max = T::enum_values().len() - 1;
        let new_ord = if self.current_param.ordinal() < max {
            self.current_param.ordinal() + 1
        } else {
            max
        };
        let new_value = T::from_ordinal(new_ord)
            .unwrap_or(T::from_ordinal(0).expect("First value of parameter"));
        self.select(new_value, stdout)
    }

    fn select_prev(&mut self, stdout: &mut RawTerminal<Stdout>) {
        let new_ord = if self.current_param.ordinal() > 0 {
            self.current_param.ordinal() - 1
        } else {
            0
        };
        let new_value = T::from_ordinal(new_ord)
            .unwrap_or(T::from_ordinal(0).expect("Last value of parameter"));
        self.select(new_value, stdout)
    }
}
