use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::process;
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::terminal_size;

#[derive(Debug)]
enum Mode {
	Normal,
	Insert,
	Command,
}

struct Editor {
	mode: Mode,
	cursor_x: usize,
	cursor_y: usize,
	buffer: Vec<String>,
	command_buffer: String,
	filename: Option<String>,
	scroll_x: usize,
	scroll_y: usize,
}

impl Editor {
	fn new() -> Self {
		Editor {
			mode: Mode::Normal,
			cursor_x: 0,
			cursor_y: 0,
			buffer: vec![String::new()],
			command_buffer: String::new(),
			filename: None,
			scroll_x: 0,
			scroll_y: 0,
		}
	}

	fn display_message(&mut self, message: String) {
		self.command_buffer = message;
	}

	fn load_file(&mut self, filename: &str) {
		if let Ok(contents) = fs::read_to_string(filename) {
			self.buffer = contents.lines().map(|line| line.to_string()).collect();
			self.filename = Some(filename.to_string());
		} else {
			self.buffer = vec![String::new()];
			self.filename = Some(filename.to_string());
		}
	}

	fn save_file(&mut self) {
		if let Some(ref filename) = self.filename {
			let mut file = OpenOptions::new()
				.write(true)
				.truncate(true)
				.create(true)
				.open(filename)
				.unwrap();

			for (i, line) in self.buffer.iter().enumerate() {
				write!(file, "{}", line).unwrap();
				if i < self.buffer.len() - 1 {
					write!(file, "\n").unwrap();
				}
			}
		} else {
			self.display_message(String::from("Error: no file to write"));
		}
	}

	fn process_key(&mut self, key: Key) {
		match self.mode {
			Mode::Normal => self.handle_normal_mode(key),
			Mode::Insert => self.handle_insert_mode(key),
			Mode::Command => self.handle_command_mode(key),
		}
	}

	fn handle_normal_mode(&mut self, key: Key) {
		match key {
			Key::Char('i') => self.mode = Mode::Insert,
			Key::Char(':') => {
				self.mode = Mode::Command;
				self.command_buffer.clear();
			}
			Key::Char('h') => {
				if self.cursor_x > 0 {
					self.cursor_x -= 1;
				} else if self.scroll_x > 0 {
					self.scroll_x -= 1;
				}
			}
			Key::Char('l') => {
				if self.cursor_x < self.buffer[self.cursor_y].len() {
					self.cursor_x += 1;
				} else if self.cursor_x >= terminal_size().unwrap().0 as usize {
					self.scroll_x += 1;
				}
			}
			Key::Char('k') => {
				if self.cursor_y > self.scroll_y {
					self.cursor_y -= 1;
					self.cursor_x = self.buffer[self.cursor_y].len().min(self.cursor_x);
				} else if self.scroll_y > 0 {
					self.scroll_y -= 1;
				}
			}
			Key::Char('j') => {
				if self.cursor_y + 1 < self.buffer.len() {
					self.cursor_y += 1;
					self.cursor_x = self.buffer[self.cursor_y].len().min(self.cursor_x);
				} else if self.cursor_y < self.buffer.len()
					&& self.cursor_y >= terminal_size().unwrap().1 as usize
				{
					self.scroll_y += 1;
				}
			}
			_ => {}
		}
	}

	fn handle_insert_mode(&mut self, key: Key) {
		match key {
			Key::Esc => self.mode = Mode::Normal,
			Key::Char('\n') => {
				let remaining_line = self.buffer[self.cursor_y].split_off(self.cursor_x);
				self.buffer.insert(self.cursor_y + 1, remaining_line);
				self.cursor_y += 1;
				self.cursor_x = 0;
				if self.cursor_y >= terminal_size().unwrap().1 as usize - 2 {
					self.scroll_y += 1;
				}
			}
			Key::Char(c) => {
				self.buffer[self.cursor_y].insert(self.cursor_x, c);
				self.cursor_x += 1;
				if self.cursor_x >= terminal_size().unwrap().0 as usize {
					self.scroll_x += 1;
				}
			}
			Key::Backspace => {
				if self.cursor_x > 0 {
					if self.cursor_x <= self.scroll_x + 1 && self.scroll_x != 0 {
						self.scroll_x -= 1;
					}
					self.cursor_x -= 1;
					self.buffer[self.cursor_y].remove(self.cursor_x);
				} else if self.cursor_y > 0 {
					let prev_line_length = self.buffer[self.cursor_y - 1].len();
					let current_line = self.buffer.remove(self.cursor_y);
					self.cursor_y -= 1;
					self.cursor_x = prev_line_length;
					self.buffer[self.cursor_y].push_str(&current_line);
				}
			}
			_ => {}
		}
	}

	fn handle_command_mode(&mut self, key: Key) {
		match key {
			Key::Esc => self.mode = Mode::Normal,
			Key::Char('\n') => self.execute_command(),
			Key::Char(c) => self.command_buffer.push(c),
			Key::Backspace => {
				self.command_buffer.pop();
			}
			_ => {}
		}
	}

	fn execute_command(&mut self) {
		let command = self.command_buffer.clone();
		match command.trim() {
			"q" => process::exit(0),
			_ if command.starts_with("w ") => {
				self.filename = Some(command.split_at(2).1.trim().to_string());
				self.save_file();
			}
			"w" => self.save_file(),
			"wq" => {
				self.save_file();
				process::exit(0);
			}
			_ if command.starts_with("e ") => {
				let filename = command.split_at(2).1.trim();
				self.load_file(filename);
			}
			_ => {}
		}
		self.mode = Mode::Normal;
	}

	fn draw(&self, stdout: &mut io::Stdout) {
		let (term_width, term_height) = terminal_size().unwrap();
		let term_height = term_height as usize;
		let term_width = term_width as usize;

		write!(stdout, "{}", termion::clear::All).unwrap();

		let start_line = self.scroll_y;
		let end_line = (self.scroll_y + term_height - 2).min(self.buffer.len());

		for (i, line) in self.buffer[start_line..end_line].iter().enumerate() {
			let visible_line = if self.scroll_x < line.len() {
				&line[self.scroll_x..(self.scroll_x + term_width).min(line.len())]
			} else {
				""
			};
			write!(stdout, "{}{}", cursor::Goto(1, i as u16 + 1), visible_line).unwrap();
		}

		let status_line = format!(
			" {:?} @ {}",
			self.mode,
			self.filename.clone().unwrap_or("[No Name]".to_string())
		);
		write!(
			stdout,
			"{}{}",
			cursor::Goto(1, term_height as u16),
			status_line
		)
		.unwrap();

		write!(
			stdout,
			"{}",
			cursor::Goto(
				(self.cursor_x - self.scroll_x + 1) as u16,
				(self.cursor_y - self.scroll_y + 1) as u16
			)
		)
		.unwrap();

		if let Mode::Command = self.mode {
			write!(
				stdout,
				"{}:{}",
				cursor::Goto(1, (term_height - 1) as u16),
				self.command_buffer
			)
			.unwrap();
		}

		stdout.flush().unwrap();
	}
}

fn main() {
	let mut editor = Editor::new();

	// Load file from arguments if provided
	if let Some(filename) = env::args().nth(1) {
		editor.load_file(&filename);
	}

	let stdin = io::stdin();
	let mut stdout = io::stdout().into_raw_mode().unwrap();
	let mut keys = stdin.keys();

	loop {
		editor.draw(&mut stdout);
		if let Some(Ok(key)) = keys.next() {
			editor.process_key(key);
		}
	}
}
