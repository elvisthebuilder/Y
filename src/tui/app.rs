use crate::protocol::message::Message;

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Timeline,
    DirectMessages,
    Communities,
    Profile,
    Compose,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
    Command,
}

pub struct App {
    pub view: View,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub timeline: Vec<Message>,
    pub dm_list: Vec<Message>,
    pub status_message: String,
    pub peer_count: usize,
    pub identity_address: String,
    pub should_quit: bool,
    pub scroll_offset: usize,
}

impl App {
    pub fn new(identity_address: String) -> Self {
        Self {
            view: View::Timeline,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            timeline: Vec::new(),
            dm_list: Vec::new(),
            status_message: "Welcome to root-chat. Press 'h' for help.".into(),
            peer_count: 0,
            identity_address,
            should_quit: false,
            scroll_offset: 0,
        }
    }

    pub fn handle_key(&mut self, key: char) {
        match self.input_mode {
            InputMode::Normal => match key {
                'q' => self.should_quit = true,
                't' => self.view = View::Timeline,
                'd' => self.view = View::DirectMessages,
                'c' => self.view = View::Communities,
                'p' => self.view = View::Profile,
                'n' => {
                    self.view = View::Compose;
                    self.input_mode = InputMode::Editing;
                }
                ':' => {
                    self.input_mode = InputMode::Command;
                    self.input_buffer.clear();
                }
                'j' => self.scroll_offset = self.scroll_offset.saturating_add(1),
                'k' => self.scroll_offset = self.scroll_offset.saturating_sub(1),
                _ => {}
            },
            InputMode::Editing => match key {
                '\x1b' => self.input_mode = InputMode::Normal,
                '\n' => {
                    self.status_message = format!("Posted: {}", self.input_buffer);
                    self.input_buffer.clear();
                    self.input_mode = InputMode::Normal;
                    self.view = View::Timeline;
                }
                _ => self.input_buffer.push(key),
            },
            InputMode::Command => match key {
                '\x1b' => {
                    self.input_mode = InputMode::Normal;
                    self.input_buffer.clear();
                }
                '\n' => {
                    self.execute_command();
                    self.input_buffer.clear();
                    self.input_mode = InputMode::Normal;
                }
                _ => self.input_buffer.push(key),
            },
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.input_buffer.trim().to_string();
        match cmd.as_str() {
            "quit" | "q" => self.should_quit = true,
            "peers" => self.status_message = format!("Connected peers: {}", self.peer_count),
            "whoami" => self.status_message = format!("Address: {}", self.identity_address),
            _ => self.status_message = format!("Unknown command: {}", cmd),
        }
    }
}
