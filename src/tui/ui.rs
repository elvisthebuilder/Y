use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::app::{App, InputMode, View};
use crate::protocol::message::MessageContent;

const ACCENT: Color = Color::Green;
const REPLY_COLOR: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_main(frame, app, chunks[1]);
    draw_input(frame, app, chunks[2]);
    draw_status(frame, app, chunks[3]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let tabs: Vec<Span> = vec![
        tab_span("Timeline[t]", app.view == View::Timeline),
        Span::raw(" | "),
        tab_span("DMs[d]", app.view == View::DirectMessages),
        Span::raw(" | "),
        tab_span("Communities[c]", app.view == View::Communities),
        Span::raw(" | "),
        tab_span("Bookmarks[b]", app.view == View::Bookmarks),
        Span::raw(" | "),
        tab_span("Profile[p]", app.view == View::Profile),
    ];

    let header = Paragraph::new(Line::from(tabs))
        .block(Block::default()
            .title(" Y ")
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT)));

    frame.render_widget(header, area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    match app.view {
        View::Timeline => draw_post_list(frame, app, &app.timeline, " Timeline ", area),
        View::DirectMessages => draw_dms(frame, app, area),
        View::Communities => draw_communities(frame, app, area),
        View::Profile => draw_profile(frame, app, area),
        View::Compose => draw_compose(frame, app, area),
        View::Search => draw_search(frame, app, area),
        View::Bookmarks => draw_post_list(frame, app, &app.bookmarks, " Bookmarks ", area),
        View::Thread => draw_post_list(frame, app, &app.timeline, " Thread ", area),
    }
}

fn draw_post_list(frame: &mut Frame, app: &App, _posts: &[crate::protocol::message::Message], title: &str, area: Rect) {
    let entries = app.visible_entries();

    let items: Vec<ListItem> = if entries.is_empty() {
        vec![ListItem::new(Span::styled(
            "  No posts. Press 'n' to compose.",
            Style::default().fg(DIM),
        ))]
    } else {
        entries.iter().enumerate().flat_map(|(i, entry)| {
            let is_selected = i == app.selected_post;
            let depth = entry.depth;

            // Build the thread line prefix for this depth
            let mut tree_prefix = String::new();
            for d in 0..depth {
                if d < entry.ancestors_continuing.len() && entry.ancestors_continuing[d] {
                    tree_prefix.push_str("│  ");
                } else {
                    tree_prefix.push_str("   ");
                }
            }

            // The connector for this node
            let connector = if depth > 0 {
                if entry.is_last_sibling {
                    "└─ "
                } else {
                    "├─ "
                }
            } else {
                ""
            };

            if entry.is_collapse_marker {
                let line = format!(
                    "  {}{}Show {} more replies...",
                    tree_prefix, connector, entry.hidden_count
                );
                let style = if is_selected {
                    Style::default().fg(ACCENT)
                } else {
                    Style::default().fg(DIM)
                };
                return vec![
                    ListItem::new(Span::styled(line, style)),
                    ListItem::new(Span::raw("")),
                ];
            }

            let msg = entry.message;
            let text = match &msg.content {
                MessageContent::Post(p) => p.text.clone(),
                MessageContent::Reply(r) => r.text.clone(),
                _ => "(other)".into(),
            };

            let author_display = if msg.author.len() > 20 {
                msg.author[..20].to_string()
            } else {
                msg.author.clone()
            };

            let select_marker = if is_selected { ">" } else { " " };

            // Line 1: tree + author
            let is_reply = depth > 0;
            let header_line = format!(
                "{} {}{}{}",
                select_marker, tree_prefix, connector, author_display
            );
            let base_color = if is_reply { REPLY_COLOR } else { ACCENT };
            let header_style = if is_selected {
                Style::default().fg(base_color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(base_color)
            };

            // Continuation prefix for lines under this node
            let continuation = if depth > 0 {
                let mut c = String::from("  ");
                for d in 0..depth {
                    if d < entry.ancestors_continuing.len() && entry.ancestors_continuing[d] {
                        c.push_str("│  ");
                    } else {
                        c.push_str("   ");
                    }
                }
                c.push_str("   ");
                c
            } else {
                "    ".to_string()
            };

            // Line 2: content
            let content_line = format!("{}{}", continuation, text);
            let content_style = if is_reply {
                Style::default().fg(REPLY_COLOR)
            } else {
                Style::default()
            };

            // Line 3: meta (with proper singular/plural)
            let is_bookmarked = app.bookmarks.iter().any(|b| b.id == msg.id);
            let nod_count = msg.nod_count();
            let reply_count = msg.reply_count();
            let nods = if nod_count == 1 {
                "1 nod".to_string()
            } else {
                format!("{} nods", nod_count)
            };
            let replies = if reply_count == 1 {
                "1 reply".to_string()
            } else {
                format!("{} replies", reply_count)
            };
            let bookmark_indicator = if is_bookmarked { " [saved]" } else { "" };
            let meta_line = format!("{}{} | {}{}", continuation, nods, replies, bookmark_indicator);

            let mut lines = vec![
                ListItem::new(Span::styled(header_line, header_style)),
                ListItem::new(Span::styled(content_line, content_style)),
                ListItem::new(Span::styled(meta_line, Style::default().fg(DIM))),
            ];

            // Add thread continuation line if this post has visible replies
            let has_replies = !entry.is_collapse_marker && msg.reply_count() > 0;
            if has_replies && depth == 0 {
                let thread_line = format!("  │");
                lines.push(ListItem::new(Span::styled(thread_line, Style::default().fg(DIM))));
            } else {
                lines.push(ListItem::new(Span::raw("")));
            }

            lines
        }).collect()
    };

    // Scroll to keep selection visible
    let lines_per_entry = 4;
    let visible_lines = area.height.saturating_sub(2) as usize;
    let selected_line = app.selected_post * lines_per_entry;
    let scroll_offset = if selected_line >= visible_lines {
        selected_line - visible_lines + lines_per_entry
    } else {
        0
    };

    let items_to_show: Vec<ListItem> = items.into_iter().skip(scroll_offset).collect();

    let help = if app.view == View::Bookmarks {
        " [.]=nod [r]=reply [s]=unsave [g]=go to [x]=delete [Enter]=expand "
    } else {
        " [.]=nod [r]=reply [s]=save [x]=delete [Enter]=expand/collapse "
    };
    let list = List::new(items_to_show)
        .block(Block::default()
            .title(title)
            .title_bottom(Line::from(Span::styled(help, Style::default().fg(DIM))))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));

    frame.render_widget(list, area);
}


fn draw_dms(frame: &mut Frame, app: &App, area: Rect) {
    let block = Paragraph::new("  End-to-end encrypted. No one else can read these.")
        .scroll((app.scroll_offset as u16, 0))
        .style(Style::default().fg(DIM))
        .block(Block::default()
            .title(" Direct Messages ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));
    frame.render_widget(block, area);
}

fn draw_communities(frame: &mut Frame, app: &App, area: Rect) {
    let block = Paragraph::new("  No communities joined. Use :join <id> to join one.")
        .scroll((app.scroll_offset as u16, 0))
        .style(Style::default().fg(DIM))
        .block(Block::default()
            .title(" Communities ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));
    frame.render_widget(block, area);
}

fn draw_profile(frame: &mut Frame, app: &App, area: Rect) {
    let info = vec![
        Line::from(vec![
            Span::styled("Handle:    ", Style::default().fg(DIM)),
            Span::styled(&app.handle, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Alias:     ", Style::default().fg(DIM)),
            Span::raw(&app.alias),
        ]),
        Line::from(vec![
            Span::styled("Address:   ", Style::default().fg(DIM)),
            Span::styled(&app.identity_address, Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::styled("Peers:     ", Style::default().fg(DIM)),
            Span::raw(format!("{}", app.peer_count)),
        ]),
        Line::from(vec![
            Span::styled("Bookmarks: ", Style::default().fg(DIM)),
            Span::raw(format!("{}", app.bookmarks.len())),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            ":alias <name>    — set your alias manually",
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            ":alias-gen       — generate a random alias",
            Style::default().fg(DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Your identity is your keypair. No email, no phone, no trace.",
            Style::default().fg(DIM),
        )),
    ];

    let block = Paragraph::new(info)
        .scroll((app.scroll_offset as u16, 0))
        .block(Block::default()
            .title(" Identity ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));
    frame.render_widget(block, area);
}

fn draw_compose(frame: &mut Frame, app: &App, area: Rect) {
    let title = " Compose (Esc=cancel, Enter=post, Shift+Enter=new line) ";
    let block = Paragraph::new(app.input_buffer.as_str())
        .block(Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT)));
    frame.render_widget(block, area);

    // Position cursor in multiline text
    let text_before_cursor = &app.input_buffer[..app.cursor_pos];
    let line_num = text_before_cursor.matches('\n').count();
    let col = text_before_cursor.rfind('\n')
        .map(|pos| app.cursor_pos - pos - 1)
        .unwrap_or(app.cursor_pos);
    frame.set_cursor_position((
        area.x + 1 + col as u16,
        area.y + 1 + line_num as u16,
    ));
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let (title, content, cursor_offset) = match app.input_mode {
        InputMode::Normal => ("Normal", String::new(), 0),
        InputMode::Editing => ("Insert", app.input_buffer.clone(), app.cursor_pos),
        InputMode::Command => ("Command", format!(":{}", app.input_buffer), app.cursor_pos + 1),
        InputMode::SearchInput => ("Search", format!("/{}", app.input_buffer), app.cursor_pos + 1),
        InputMode::Replying => ("Reply", app.input_buffer.clone(), app.cursor_pos),
    };

    let border_color = match app.input_mode {
        InputMode::Normal => DIM,
        InputMode::Editing => ACCENT,
        InputMode::Command => Color::Yellow,
        InputMode::SearchInput => Color::Cyan,
        InputMode::Replying => Color::Magenta,
    };

    let input = Paragraph::new(content)
        .block(Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)));

    frame.render_widget(input, area);

    // Place the visible cursor when in an input mode (but not Editing — compose handles its own)
    if app.input_mode != InputMode::Normal && app.input_mode != InputMode::Editing {
        frame.set_cursor_position((
            area.x + 1 + cursor_offset as u16,
            area.y + 1,
        ));
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let status = Paragraph::new(Span::styled(
        &app.status_message,
        Style::default().fg(DIM),
    ));
    frame.render_widget(status, area);
}

fn draw_search(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Search: ", Style::default().fg(DIM)),
            Span::styled(&app.input_buffer, Style::default().fg(ACCENT)),
            Span::styled("_", Style::default().fg(ACCENT)),
        ]),
        Line::from(""),
    ];

    if app.search_results.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Type an alias or address to find users. Enter to search.",
            Style::default().fg(DIM),
        )));
    } else {
        for result in &app.search_results {
            lines.push(Line::from(Span::styled(result, Style::default().fg(ACCENT))));
        }
    }

    let block = Paragraph::new(lines)
        .block(Block::default()
            .title(" Search Users [/] ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT)));
    frame.render_widget(block, area);
}

fn tab_span(label: &str, active: bool) -> Span<'_> {
    if active {
        Span::styled(label, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(label, Style::default().fg(DIM))
    }
}
