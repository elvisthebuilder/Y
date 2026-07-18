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
const DIM: Color = Color::DarkGray;
const NOD_COLOR: Color = Color::Yellow;

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
        View::Thread => draw_thread(frame, app, area),
    }
}

fn draw_post_list(frame: &mut Frame, app: &App, posts: &[crate::protocol::message::Message], title: &str, area: Rect) {
    let items: Vec<ListItem> = if posts.is_empty() {
        vec![ListItem::new(Span::styled(
            "  No posts. Press 'n' to compose.",
            Style::default().fg(DIM),
        ))]
    } else {
        posts.iter().enumerate().map(|(i, msg)| {
            let is_selected = i == app.selected_post;
            let text = match &msg.content {
                MessageContent::Post(p) => p.text.clone(),
                MessageContent::Reply(r) => format!("-> {}", r.text),
                _ => "(other)".into(),
            };

            let author_display = if msg.author.len() > 20 {
                msg.author[..20].to_string()
            } else {
                msg.author.clone()
            };

            let nod_str = if msg.nod_count() > 0 {
                format!(" [{}N]", msg.nod_count())
            } else {
                String::new()
            };

            let reply_str = if msg.reply_count() > 0 {
                format!(" [{}R]", msg.reply_count())
            } else {
                String::new()
            };

            let prefix = if is_selected { "> " } else { "  " };
            let formatted = format!("{}{} {}{}{}", prefix, author_display, text, nod_str, reply_str);

            let style = if is_selected {
                Style::default().fg(ACCENT)
            } else {
                Style::default()
            };

            ListItem::new(Span::styled(formatted, style))
        }).collect()
    };

    let help = " [.]=nod [r]=reply [s]=bookmark [Enter]=thread ";
    let list = List::new(items)
        .block(Block::default()
            .title(title)
            .title_bottom(Line::from(Span::styled(help, Style::default().fg(DIM))))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));

    frame.render_widget(list, area);
}

fn draw_thread(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(parent) = app.timeline.get(app.selected_post) {
        let text = match &parent.content {
            MessageContent::Post(p) => p.text.clone(),
            MessageContent::Reply(r) => r.text.clone(),
            _ => "(other)".into(),
        };
        let header = format!("{} {}", parent.author, text);
        lines.push(Line::from(Span::styled(header, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))));
        lines.push(Line::from(Span::styled(
            format!("  {} nods | {} replies", parent.nod_count(), parent.reply_count()),
            Style::default().fg(DIM),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("--- replies ---", Style::default().fg(DIM))));
        lines.push(Line::from(""));
    }

    for reply in &app.thread_replies {
        let text = match &reply.content {
            MessageContent::Reply(r) => r.text.clone(),
            MessageContent::Post(p) => p.text.clone(),
            _ => "(other)".into(),
        };
        let line_str = format!("  {}: {}", reply.author, text);
        lines.push(Line::from(Span::styled(line_str, Style::default().fg(ACCENT))));
    }

    if app.thread_replies.is_empty() {
        lines.push(Line::from(Span::styled("  No replies yet. Press 'r' to reply.", Style::default().fg(DIM))));
    }

    let block = Paragraph::new(lines)
        .block(Block::default()
            .title(" Thread (Esc to go back) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));
    frame.render_widget(block, area);
}

fn draw_dms(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Paragraph::new("  End-to-end encrypted. No one else can read these.")
        .style(Style::default().fg(DIM))
        .block(Block::default()
            .title(" Direct Messages ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));
    frame.render_widget(block, area);
}

fn draw_communities(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Paragraph::new("  No communities joined. Use :join <id> to join one.")
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
        .block(Block::default()
            .title(" Identity ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));
    frame.render_widget(block, area);
}

fn draw_compose(frame: &mut Frame, app: &App, area: Rect) {
    let block = Paragraph::new(app.input_buffer.as_str())
        .block(Block::default()
            .title(" Compose (Esc to cancel, Enter to post) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT)));
    frame.render_widget(block, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let (title, content) = match app.input_mode {
        InputMode::Normal => ("Normal", String::new()),
        InputMode::Editing => ("Insert", app.input_buffer.clone()),
        InputMode::Command => ("Command", format!(":{}", app.input_buffer)),
        InputMode::SearchInput => ("Search", format!("/{}", app.input_buffer)),
        InputMode::Replying => ("Reply", app.input_buffer.clone()),
    };

    let input = Paragraph::new(content)
        .block(Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(match app.input_mode {
                InputMode::Normal => DIM,
                InputMode::Editing => ACCENT,
                InputMode::Command => Color::Yellow,
                InputMode::SearchInput => Color::Cyan,
                InputMode::Replying => Color::Magenta,
            })));

    frame.render_widget(input, area);
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
