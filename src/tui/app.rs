use chrono::Utc;
use rand;
use crate::protocol::message::{Message, MessageContent, PostMessage, ReplyMessage, Nod};

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Timeline,
    DirectMessages,
    Communities,
    Profile,
    Compose,
    Search,
    Bookmarks,
    Thread,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
    Command,
    SearchInput,
    Replying,
}

pub struct App {
    pub view: View,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub timeline: Vec<Message>,
    pub dm_list: Vec<Message>,
    pub bookmarks: Vec<Message>,
    pub status_message: String,
    pub peer_count: usize,
    pub identity_address: String,
    pub handle: String,
    pub alias: String,
    pub should_quit: bool,
    pub scroll_offset: usize,
    pub selected_post: usize,
    pub search_results: Vec<String>,
    pub pending_alias_change: Option<String>,
    pub pending_post: bool,
    pub pending_nod: Option<String>,
    pub pending_bookmark: Option<(String, bool)>,
    pub pending_save: bool,
    pub thread_replies: Vec<Message>,
}

impl App {
    pub fn new(identity_address: String, handle: String, alias: String) -> Self {
        Self {
            view: View::Timeline,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            timeline: Vec::new(),
            dm_list: Vec::new(),
            bookmarks: Vec::new(),
            status_message: format!("Welcome to Y. You are {}", handle),
            peer_count: 0,
            identity_address,
            handle,
            alias,
            should_quit: false,
            scroll_offset: 0,
            selected_post: 0,
            search_results: Vec::new(),
            pending_alias_change: None,
            pending_post: false,
            pending_nod: None,
            pending_bookmark: None,
            pending_save: false,
            thread_replies: Vec::new(),
        }
    }

    fn selected_message(&self) -> Option<&Message> {
        let list = match self.view {
            View::Bookmarks => &self.bookmarks,
            _ => &self.timeline,
        };
        list.get(self.selected_post)
    }

    fn selected_message_mut(&mut self) -> Option<&mut Message> {
        let list = match self.view {
            View::Bookmarks => &mut self.bookmarks,
            _ => &mut self.timeline,
        };
        list.get_mut(self.selected_post)
    }

    pub fn handle_key(&mut self, key: char) {
        match self.input_mode {
            InputMode::Normal => match key {
                'q' => self.should_quit = true,
                't' => { self.view = View::Timeline; self.selected_post = 0; self.scroll_offset = 0; }
                'd' => { self.view = View::DirectMessages; self.scroll_offset = 0; }
                'c' => { self.view = View::Communities; self.scroll_offset = 0; }
                'p' => { self.view = View::Profile; self.scroll_offset = 0; }
                'b' => {
                    self.view = View::Bookmarks;
                    self.selected_post = 0;
                    self.scroll_offset = 0;
                }
                '/' => {
                    self.view = View::Search;
                    self.input_mode = InputMode::SearchInput;
                    self.input_buffer.clear();
                    self.search_results.clear();
                }
                'n' => {
                    self.view = View::Compose;
                    self.input_mode = InputMode::Editing;
                    self.input_buffer.clear();
                }
                'r' => {
                    if self.selected_message().is_some() {
                        self.input_mode = InputMode::Replying;
                        self.input_buffer.clear();
                    }
                }
                '.' => {
                    self.nod_selected();
                }
                's' => {
                    self.bookmark_selected();
                }
                '\n' => {
                    self.open_thread();
                }
                ':' => {
                    self.input_mode = InputMode::Command;
                    self.input_buffer.clear();
                }
                'j' => {
                    match self.view {
                        View::Timeline | View::Bookmarks => {
                            let max = match self.view {
                                View::Bookmarks => self.bookmarks.len(),
                                _ => self.timeline.len(),
                            };
                            if self.selected_post + 1 < max {
                                self.selected_post += 1;
                            }
                        }
                        _ => {
                            self.scroll_offset = self.scroll_offset.saturating_add(1);
                        }
                    }
                }
                'k' => {
                    match self.view {
                        View::Timeline | View::Bookmarks => {
                            self.selected_post = self.selected_post.saturating_sub(1);
                        }
                        _ => {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        }
                    }
                }
                _ => {}
            },
            InputMode::Editing => match key {
                '\x1b' => {
                    self.input_mode = InputMode::Normal;
                    self.view = View::Timeline;
                }
                '\n' => {
                    if !self.input_buffer.trim().is_empty() {
                        let msg = Message {
                            id: format!("{:x}", rand::random::<u64>()),
                            author: self.handle.clone(),
                            content: MessageContent::Post(PostMessage {
                                text: self.input_buffer.clone(),
                                media: None,
                            }),
                            timestamp: Utc::now(),
                            signature: Vec::new(),
                            reply_to: None,
                            nods: Vec::new(),
                            replies: Vec::new(),
                        };
                        self.timeline.insert(0, msg);
                        self.pending_post = true;
                        self.status_message = "Post published.".into();
                    }
                    self.input_buffer.clear();
                    self.input_mode = InputMode::Normal;
                    self.view = View::Timeline;
                }
                _ => self.input_buffer.push(key),
            },
            InputMode::Replying => match key {
                '\x1b' => {
                    self.input_mode = InputMode::Normal;
                    self.input_buffer.clear();
                }
                '\n' => {
                    self.submit_reply();
                    self.input_buffer.clear();
                    self.input_mode = InputMode::Normal;
                }
                _ => self.input_buffer.push(key),
            },
            InputMode::SearchInput => match key {
                '\x1b' => {
                    self.input_mode = InputMode::Normal;
                    self.view = View::Timeline;
                    self.input_buffer.clear();
                }
                '\n' => {
                    self.status_message = format!("Searching for '{}'...", self.input_buffer);
                    self.input_mode = InputMode::Normal;
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

    fn nod_selected(&mut self) {
        let handle = self.handle.clone();
        let list = match self.view {
            View::Bookmarks => &mut self.bookmarks,
            _ => &mut self.timeline,
        };
        if let Some(msg) = list.get_mut(self.selected_post) {
            if !msg.has_nodded(&handle) {
                msg.nods.push(Nod {
                    from: handle,
                    timestamp: Utc::now(),
                });
                let count = msg.nod_count();
                let id = msg.id.clone();
                self.pending_nod = Some(id);
                self.pending_save = true;
                self.status_message = format!("Nodded. ({} nods)", count);
            } else {
                self.status_message = "Already nodded.".into();
            }
        }
    }

    fn bookmark_selected(&mut self) {
        if let Some(msg) = self.selected_message() {
            let id = msg.id.clone();
            let msg_clone = msg.clone();
            // Toggle bookmark
            if self.bookmarks.iter().any(|b| b.id == id) {
                self.bookmarks.retain(|b| b.id != id);
                self.pending_bookmark = Some((id, false));
                self.status_message = "Bookmark removed.".into();
            } else {
                self.bookmarks.push(msg_clone);
                self.pending_bookmark = Some((id, true));
                self.status_message = "Post bookmarked.".into();
            }
        }
    }

    fn submit_reply(&mut self) {
        if self.input_buffer.trim().is_empty() {
            return;
        }
        if let Some(parent) = self.selected_message() {
            let parent_id = parent.id.clone();
            let reply = Message {
                id: format!("{:x}", rand::random::<u64>()),
                author: self.handle.clone(),
                content: MessageContent::Reply(ReplyMessage {
                    parent_id: parent_id.clone(),
                    text: self.input_buffer.clone(),
                }),
                timestamp: Utc::now(),
                signature: Vec::new(),
                reply_to: Some(parent_id.clone()),
                nods: Vec::new(),
                replies: Vec::new(),
            };
            let reply_id = reply.id.clone();

            // Add reply to timeline
            self.timeline.insert(0, reply);
            self.pending_post = true;

            // Track reply on parent
            if let Some(parent_msg) = self.timeline.iter_mut().find(|m| m.id == parent_id) {
                parent_msg.replies.push(reply_id);
                self.pending_save = true;
            }

            self.status_message = "Reply posted.".into();
        }
    }

    fn open_thread(&mut self) {
        if let Some(msg) = self.selected_message() {
            let parent_id = msg.id.clone();
            let reply_ids = msg.replies.clone();

            self.thread_replies = self.timeline.iter()
                .filter(|m| reply_ids.contains(&m.id) || m.reply_to.as_deref() == Some(&parent_id))
                .cloned()
                .collect();
            self.thread_replies.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            self.view = View::Thread;
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.input_buffer.trim().to_string();
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();

        match parts[0] {
            "quit" | "q" => self.should_quit = true,
            "peers" => self.status_message = format!("Connected peers: {}", self.peer_count),
            "whoami" => self.status_message = format!("{} ({})", self.handle, self.identity_address),
            "alias" => {
                if parts.len() > 1 {
                    let new_alias = parts[1].to_string();
                    self.alias = new_alias.clone();
                    self.handle = format!("{}#{}", new_alias, &self.identity_address.strip_prefix("root:").unwrap_or(&self.identity_address).chars().take(4).collect::<String>());
                    self.pending_alias_change = Some(new_alias.clone());
                    self.status_message = format!("Alias changed to: {}", self.handle);
                } else {
                    self.status_message = format!("Current alias: {}", self.handle);
                }
            }
            "alias-gen" => {
                let new_alias = crate::crypto::alias::generate_alias();
                self.alias = new_alias.clone();
                self.handle = format!("{}#{}", new_alias, &self.identity_address.strip_prefix("root:").unwrap_or(&self.identity_address).chars().take(4).collect::<String>());
                self.pending_alias_change = Some(new_alias.clone());
                self.status_message = format!("Generated new alias: {}", self.handle);
            }
            "search" => {
                if parts.len() > 1 {
                    self.view = View::Search;
                    self.status_message = format!("Searching for '{}'...", parts[1]);
                } else {
                    self.status_message = "Usage: :search <alias or address>".into();
                }
            }
            _ => self.status_message = format!("Unknown command: {}", cmd),
        }
    }
}
