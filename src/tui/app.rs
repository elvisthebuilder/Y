use crate::community::{Community, JoinResult};
use crate::protocol::message::{Message, MessageContent, Nod, PostMessage, ReplyMessage};
use chrono::Utc;
use rand;
use std::collections::HashSet;

pub struct DisplayEntry<'a> {
    pub message: &'a Message,
    pub depth: usize,
    pub is_collapse_marker: bool,
    pub hidden_count: usize,
    pub ancestors_continuing: Vec<bool>,
    pub is_last_sibling: bool,
}

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
    CommunityDetail,
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
    pub cursor_pos: usize,
    pub timeline: Vec<Message>,
    pub dm_list: Vec<Message>,
    pub bookmarks: Vec<Message>,
    pub status_message: String,
    pub peer_count: usize,
    pub onion_address: Option<String>,
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
    pub pending_copy: Option<String>,
    pub pending_deletes: Vec<String>,
    pub confirm_delete: Option<String>,
    pub thread_replies: Vec<Message>,
    pub expanded_threads: HashSet<String>,
    pub max_visible_replies: usize,
    pub communities: Vec<Community>,
    pub selected_community: Option<usize>,
    pub selected_list_item: usize,
    pub known_users: Vec<(String, String)>,
}

impl App {
    pub fn new(identity_address: String, handle: String, alias: String) -> Self {
        Self {
            view: View::Timeline,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            cursor_pos: 0,
            timeline: Vec::new(),
            dm_list: Vec::new(),
            bookmarks: Vec::new(),
            status_message: format!("Welcome to Y. You are {}", handle),
            peer_count: 0,
            onion_address: None,
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
            pending_copy: None,
            pending_deletes: Vec::new(),
            confirm_delete: None,
            thread_replies: Vec::new(),
            expanded_threads: HashSet::new(),
            max_visible_replies: 2,
            communities: Vec::new(),
            selected_community: None,
            selected_list_item: 0,
            known_users: Vec::new(),
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.input_buffer.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_char_before_cursor(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input_buffer[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_buffer.remove(prev);
            self.cursor_pos = prev;
            if self.input_mode == InputMode::SearchInput {
                self.update_search_results();
            }
        } else if matches!(
            self.input_mode,
            InputMode::Command | InputMode::SearchInput | InputMode::Replying
        ) {
            self.input_mode = InputMode::Normal;
            if matches!(self.view, View::Search) {
                self.view = View::Timeline;
                self.search_results.clear();
            }
        }
    }

    pub fn delete_char_at_cursor(&mut self) {
        if self.cursor_pos < self.input_buffer.len() {
            self.input_buffer.remove(self.cursor_pos);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.input_buffer[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.input_buffer.len() {
            self.cursor_pos = self.input_buffer[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.input_buffer.len());
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input_buffer.len();
    }

    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_pos = 0;
    }

    pub fn visible_entries(&self) -> Vec<DisplayEntry<'_>> {
        let posts = match self.view {
            View::Bookmarks => &self.bookmarks,
            _ => &self.timeline,
        };
        let top_level: Vec<&Message> = posts.iter().filter(|m| m.reply_to.is_none()).collect();

        let mut entries = Vec::new();
        for msg in &top_level {
            self.build_thread_entries(posts, msg, 0, &mut entries, &[]);
        }
        entries
    }

    fn build_thread_entries<'a>(
        &self,
        all_posts: &'a [Message],
        msg: &'a Message,
        depth: usize,
        entries: &mut Vec<DisplayEntry<'a>>,
        ancestors_continuing: &[bool],
    ) {
        entries.push(DisplayEntry {
            message: msg,
            depth,
            is_collapse_marker: false,
            hidden_count: 0,
            ancestors_continuing: ancestors_continuing.to_vec(),
            is_last_sibling: false,
        });

        let replies: Vec<&Message> = all_posts
            .iter()
            .filter(|m| m.reply_to.as_deref() == Some(&msg.id))
            .collect();

        if replies.is_empty() {
            return;
        }

        let is_expanded = self.expanded_threads.contains(&msg.id);
        if !is_expanded {
            // Collapsed: show nothing, just add collapse marker
            entries.push(DisplayEntry {
                message: msg,
                depth: depth + 1,
                is_collapse_marker: true,
                hidden_count: replies.len(),
                ancestors_continuing: {
                    let mut a = ancestors_continuing.to_vec();
                    a.push(false);
                    a
                },
                is_last_sibling: true,
            });
            return;
        }

        for (i, reply) in replies.iter().enumerate() {
            let is_last = i == replies.len() - 1;
            let mut new_ancestors = ancestors_continuing.to_vec();
            new_ancestors.push(!is_last);

            let idx = entries.len();
            self.build_thread_entries(all_posts, reply, depth + 1, entries, &new_ancestors);
            entries[idx].is_last_sibling = is_last;
        }
    }

    fn selected_message_id(&self) -> Option<String> {
        let entries = self.visible_entries();
        entries
            .get(self.selected_post)
            .map(|e| e.message.id.clone())
    }

    fn selected_message_mut(&mut self) -> Option<&mut Message> {
        let id = self.selected_message_id();
        if let Some(id) = id {
            self.timeline.iter_mut().find(|m| m.id == id)
        } else {
            None
        }
    }

    pub fn handle_key(&mut self, key: char) {
        // Handle delete confirmation if active
        if self.confirm_delete.is_some() && self.input_mode == InputMode::Normal {
            match key {
                'y' => self.delete_selected_post(),
                _ => {
                    self.confirm_delete = None;
                    self.status_message = "Delete cancelled.".into();
                }
            }
            return;
        }

        match self.input_mode {
            InputMode::Normal => match key {
                'q' => self.should_quit = true,
                't' => {
                    self.view = View::Timeline;
                    self.selected_post = 0;
                    self.scroll_offset = 0;
                }
                'd' => {
                    self.view = View::DirectMessages;
                    self.scroll_offset = 0;
                }
                'c' => {
                    self.view = View::Communities;
                    self.scroll_offset = 0;
                    self.selected_list_item = 0;
                    self.selected_community = None;
                }
                'p' => {
                    self.view = View::Profile;
                    self.scroll_offset = 0;
                }
                'y' if self.view == View::Profile => {
                    if let Some(ref addr) = self.onion_address {
                        self.pending_copy = Some(addr.clone());
                        self.status_message = "Onion address copied to clipboard".into();
                    } else {
                        self.status_message = "Onion address not ready yet".into();
                    }
                }
                'b' => {
                    self.view = View::Bookmarks;
                    self.selected_post = 0;
                    self.scroll_offset = 0;
                }
                '/' => {
                    self.view = View::Search;
                    self.input_mode = InputMode::SearchInput;
                    self.clear_input();
                    self.search_results.clear();
                }
                'n' => {
                    self.view = View::Compose;
                    self.input_mode = InputMode::Editing;
                    self.clear_input();
                }
                'r' if self.selected_message_id().is_some() => {
                    self.input_mode = InputMode::Replying;
                    self.clear_input();
                }
                '.' => {
                    self.nod_selected();
                }
                's' => {
                    self.bookmark_selected();
                }
                'g' => {
                    self.goto_post_in_timeline();
                }
                'x' if self.view != View::CommunityDetail => {
                    self.prompt_delete();
                }
                '\n' => match self.view {
                    View::Communities => {
                        if !self.communities.is_empty() {
                            self.selected_community = Some(self.selected_list_item);
                            self.view = View::CommunityDetail;
                            self.selected_list_item = 0;
                        }
                    }
                    _ => {
                        self.open_thread();
                    }
                },
                '\x1b' if self.view == View::CommunityDetail => {
                    self.view = View::Communities;
                    self.selected_list_item = self.selected_community.unwrap_or(0);
                    self.selected_community = None;
                }
                'a' if self.view == View::CommunityDetail => {
                    self.approve_selected_request();
                }
                'x' if self.view == View::CommunityDetail => {
                    self.decline_selected_request();
                }
                ':' => {
                    self.input_mode = InputMode::Command;
                    self.clear_input();
                }
                'j' => match self.view {
                    View::Timeline | View::Bookmarks => {
                        let max = self.visible_entries().len();
                        if self.selected_post + 1 < max {
                            self.selected_post += 1;
                        }
                    }
                    View::Communities => {
                        if self.selected_list_item + 1 < self.communities.len() {
                            self.selected_list_item += 1;
                        }
                    }
                    View::CommunityDetail => {
                        let max = self.community_detail_item_count();
                        if self.selected_list_item + 1 < max {
                            self.selected_list_item += 1;
                        }
                    }
                    _ => {
                        self.scroll_offset = self.scroll_offset.saturating_add(1);
                    }
                },
                'k' => match self.view {
                    View::Timeline | View::Bookmarks => {
                        self.selected_post = self.selected_post.saturating_sub(1);
                    }
                    View::Communities | View::CommunityDetail => {
                        self.selected_list_item = self.selected_list_item.saturating_sub(1);
                    }
                    _ => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    }
                },
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
                    self.clear_input();
                    self.input_mode = InputMode::Normal;
                    self.view = View::Timeline;
                }
                _ => self.insert_char(key),
            },
            InputMode::Replying => match key {
                '\x1b' => {
                    self.input_mode = InputMode::Normal;
                    self.clear_input();
                }
                '\n' => {
                    self.submit_reply();
                    self.clear_input();
                    self.input_mode = InputMode::Normal;
                }
                _ => self.insert_char(key),
            },
            InputMode::SearchInput => match key {
                '\x1b' => {
                    self.input_mode = InputMode::Normal;
                    self.view = View::Timeline;
                    self.clear_input();
                    self.search_results.clear();
                }
                '\n' => {
                    self.input_mode = InputMode::Normal;
                }
                _ => {
                    self.insert_char(key);
                    self.update_search_results();
                }
            },
            InputMode::Command => match key {
                '\x1b' => {
                    self.input_mode = InputMode::Normal;
                    self.clear_input();
                }
                '\n' => {
                    self.execute_command();
                    self.clear_input();
                    self.input_mode = InputMode::Normal;
                }
                _ => self.insert_char(key),
            },
        }
    }

    fn goto_post_in_timeline(&mut self) {
        if self.view != View::Bookmarks {
            return;
        }
        let id = match self.selected_message_id() {
            Some(id) => id,
            None => return,
        };

        // Switch to timeline and find the post in visible entries
        self.view = View::Timeline;
        self.scroll_offset = 0;

        let entries = self.visible_entries();
        if let Some(pos) = entries.iter().position(|e| e.message.id == id) {
            self.selected_post = pos;
            self.status_message = "Jumped to post in timeline.".into();
        } else {
            self.selected_post = 0;
            self.status_message = "Post not found in timeline.".into();
        }
    }

    fn prompt_delete(&mut self) {
        let id = match self.selected_message_id() {
            Some(id) => id,
            None => return,
        };
        if let Some(msg) = self.timeline.iter().find(|m| m.id == id) {
            if msg.author != self.handle {
                self.status_message = "You can only delete your own posts.".into();
                return;
            }
        } else {
            return;
        }
        self.confirm_delete = Some(id);
        self.status_message = "Delete this post? (y to confirm, any other key to cancel)".into();
    }

    fn delete_selected_post(&mut self) {
        let id = match self.confirm_delete.take() {
            Some(id) => id,
            None => return,
        };

        // Remove from timeline
        self.timeline.retain(|m| m.id != id);

        // Also remove any replies to this post
        self.timeline
            .retain(|m| m.reply_to.as_deref() != Some(id.as_str()));

        // Remove from bookmarks if bookmarked
        self.bookmarks.retain(|m| m.id != id);

        // Remove reply reference from parent if this was a reply
        for msg in &mut self.timeline {
            msg.replies.retain(|r| r != &id);
        }

        // Adjust selection
        let max = self.visible_entries().len();
        if self.selected_post >= max && max > 0 {
            self.selected_post = max - 1;
        }

        self.pending_deletes.push(id);
        self.pending_save = true;
        self.status_message = "Post deleted.".into();
    }

    fn nod_selected(&mut self) {
        let handle = self.handle.clone();
        let id = match self.selected_message_id() {
            Some(id) => id,
            None => return,
        };
        if let Some(msg) = self.timeline.iter_mut().find(|m| m.id == id) {
            if !msg.has_nodded(&handle) {
                msg.nods.push(Nod {
                    from: handle,
                    timestamp: Utc::now(),
                });
                let count = msg.nod_count();
                self.pending_nod = Some(id);
                self.pending_save = true;
                self.status_message = format!("Nodded. ({} nods)", count);
            } else {
                self.status_message = "Already nodded.".into();
            }
        }
    }

    fn bookmark_selected(&mut self) {
        let id = match self.selected_message_id() {
            Some(id) => id,
            None => return,
        };
        if self.bookmarks.iter().any(|b| b.id == id) {
            self.bookmarks.retain(|b| b.id != id);
            self.pending_bookmark = Some((id, false));
            self.status_message = "Bookmark removed.".into();
        } else {
            if let Some(msg) = self.timeline.iter().find(|m| m.id == id) {
                self.bookmarks.push(msg.clone());
            }
            self.pending_bookmark = Some((id, true));
            self.status_message = "Post bookmarked.".into();
        }
    }

    fn submit_reply(&mut self) {
        if self.input_buffer.trim().is_empty() {
            return;
        }
        let parent_id = match self.selected_message_id() {
            Some(id) => id,
            None => return,
        };

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

        self.timeline.push(reply);
        self.pending_post = true;

        if let Some(parent_msg) = self.timeline.iter_mut().find(|m| m.id == parent_id) {
            parent_msg.replies.push(reply_id);
            self.pending_save = true;
        }

        // Auto-expand the thread we just replied to
        self.expanded_threads.insert(parent_id);
        self.status_message = "Reply posted.".into();
    }

    fn open_thread(&mut self) {
        let entries = self.visible_entries();
        if let Some(entry) = entries.get(self.selected_post) {
            if entry.is_collapse_marker {
                let parent_id = entry.message.id.clone();
                self.expanded_threads.insert(parent_id);
                self.status_message = "Thread expanded.".into();
            } else {
                let id = entry.message.id.clone();
                // Check if this post actually has replies in the timeline
                let has_child_replies = self
                    .timeline
                    .iter()
                    .any(|m| m.reply_to.as_deref() == Some(id.as_str()));
                if !has_child_replies {
                    self.status_message = "No replies to expand.".into();
                    return;
                }
                if self.expanded_threads.contains(&id) {
                    self.expanded_threads.remove(&id);
                    self.status_message = "Thread collapsed.".into();
                } else {
                    self.expanded_threads.insert(id);
                    self.status_message = "Thread expanded.".into();
                }
            }
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.input_buffer.trim().to_string();
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();

        match parts[0] {
            "quit" | "q" => self.should_quit = true,
            "peers" => self.status_message = format!("Connected peers: {}", self.peer_count),
            "whoami" => {
                self.status_message = format!("{} ({})", self.handle, self.identity_address)
            }
            "alias" => {
                if parts.len() > 1 {
                    let new_alias = parts[1].to_string();
                    self.alias = new_alias.clone();
                    self.handle = format!(
                        "{}#{}",
                        new_alias,
                        self.identity_address
                            .strip_prefix("root:")
                            .unwrap_or(&self.identity_address)
                            .chars()
                            .take(4)
                            .collect::<String>()
                    );
                    self.pending_alias_change = Some(new_alias.clone());
                    self.status_message = format!("Alias changed to: {}", self.handle);
                } else {
                    self.status_message = format!("Current alias: {}", self.handle);
                }
            }
            "alias-gen" => {
                let new_alias = crate::crypto::alias::generate_alias();
                self.alias = new_alias.clone();
                self.handle = format!(
                    "{}#{}",
                    new_alias,
                    self.identity_address
                        .strip_prefix("root:")
                        .unwrap_or(&self.identity_address)
                        .chars()
                        .take(4)
                        .collect::<String>()
                );
                self.pending_alias_change = Some(new_alias.clone());
                self.status_message = format!("Generated new alias: {}", self.handle);
            }
            "create" => {
                if parts.len() > 1 {
                    let args: Vec<&str> = parts[1].splitn(2, ' ').collect();
                    let name = args[0].to_string();
                    let is_private = args.len() > 1 && args[1].eq_ignore_ascii_case("private");
                    if self.communities.iter().any(|c| c.name == name) {
                        self.status_message = format!("Community '{}' already exists.", name);
                    } else {
                        let label = if is_private { "private" } else { "open" };
                        let community = Community::new(
                            name.clone(),
                            String::new(),
                            self.identity_address.clone(),
                            is_private,
                        );
                        self.status_message =
                            format!("Created {} community '{}' ({})", label, name, community.id);
                        self.communities.push(community);
                        self.view = View::Communities;
                        self.selected_list_item = self.communities.len() - 1;
                    }
                } else {
                    self.status_message = "Usage: :create <name> or :create <name> private".into();
                }
            }
            "join" => {
                if parts.len() > 1 {
                    let id = parts[1].to_string();
                    let addr = self.identity_address.clone();
                    if let Some(community) = self.communities.iter_mut().find(|c| c.id == id) {
                        match community.request_join(&addr) {
                            JoinResult::Joined => {
                                self.status_message =
                                    format!("Joined community '{}'.", community.name);
                            }
                            JoinResult::Pending => {
                                self.status_message = format!(
                                    "Join request sent for '{}'. Waiting for owner approval.",
                                    community.name
                                );
                            }
                            JoinResult::AlreadyMember => {
                                self.status_message =
                                    format!("Already a member of '{}'.", community.name);
                            }
                            JoinResult::AlreadyPending => {
                                self.status_message =
                                    format!("Request already pending for '{}'.", community.name);
                            }
                        }
                    } else {
                        self.status_message = format!("Community '{}' not found.", id);
                    }
                    self.view = View::Communities;
                } else {
                    self.status_message = "Usage: :join <id>".into();
                }
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

    fn community_detail_item_count(&self) -> usize {
        if let Some(idx) = self.selected_community {
            if let Some(community) = self.communities.get(idx) {
                let is_owner = community.owner == self.identity_address;
                let requests = if is_owner {
                    community.pending_requests.len()
                } else {
                    0
                };
                let members = community.members.len();
                return requests + members;
            }
        }
        0
    }

    fn approve_selected_request(&mut self) {
        let idx = match self.selected_community {
            Some(i) => i,
            None => return,
        };
        let owner = self.identity_address.clone();
        let community = match self.communities.get(idx) {
            Some(c) => c,
            None => return,
        };
        if community.owner != owner {
            self.status_message = "Only the owner can approve requests.".into();
            return;
        }
        let request_count = community.pending_requests.len();
        if self.selected_list_item >= request_count {
            self.status_message = "No request selected.".into();
            return;
        }
        let address = community.pending_requests[self.selected_list_item].clone();
        let community = &mut self.communities[idx];
        community.approve_request(&address, &owner);
        self.status_message = format!("Approved {}.", truncate_address(&address));
        let max = self.community_detail_item_count();
        if self.selected_list_item >= max && max > 0 {
            self.selected_list_item = max - 1;
        }
    }

    fn decline_selected_request(&mut self) {
        let idx = match self.selected_community {
            Some(i) => i,
            None => return,
        };
        let owner = self.identity_address.clone();
        let community = match self.communities.get(idx) {
            Some(c) => c,
            None => return,
        };
        if community.owner != owner {
            self.status_message = "Only the owner can decline requests.".into();
            return;
        }
        let request_count = community.pending_requests.len();
        if self.selected_list_item >= request_count {
            self.status_message = "No request selected.".into();
            return;
        }
        let address = community.pending_requests[self.selected_list_item].clone();
        let community = &mut self.communities[idx];
        community.decline_request(&address, &owner);
        self.status_message = format!("Declined {}.", truncate_address(&address));
        let max = self.community_detail_item_count();
        if self.selected_list_item >= max && max > 0 {
            self.selected_list_item = max - 1;
        }
    }

    fn update_search_results(&mut self) {
        let query = self.input_buffer.to_lowercase();
        self.search_results.clear();
        if query.is_empty() {
            return;
        }
        for (alias, address) in &self.known_users {
            if alias.to_lowercase().contains(&query) || address.to_lowercase().contains(&query) {
                self.search_results
                    .push(format!("{}  {}", alias, truncate_address(address)));
            }
        }
    }

    pub fn add_known_user(&mut self, alias: String, address: String) {
        if !self.known_users.iter().any(|(_, a)| a == &address) {
            self.known_users.push((alias, address));
        }
    }
}

fn truncate_address(addr: &str) -> String {
    if addr.len() > 20 {
        format!("{}...{}", &addr[..10], &addr[addr.len() - 6..])
    } else {
        addr.to_string()
    }
}
