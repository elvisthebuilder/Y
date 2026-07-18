use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::app::{App, InputMode, View};

const ACCENT: Color = Color::Green;
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
        tab_span("Profile[p]", app.view == View::Profile),
    ];

    let header = Paragraph::new(Line::from(tabs))
        .block(Block::default()
            .title(" root-chat ")
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT)));

    frame.render_widget(header, area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    match app.view {
        View::Timeline => draw_timeline(frame, app, area),
        View::DirectMessages => draw_dms(frame, app, area),
        View::Communities => draw_communities(frame, app, area),
        View::Profile => draw_profile(frame, app, area),
        View::Compose => draw_compose(frame, app, area),
    }
}

fn draw_timeline(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.timeline.is_empty() {
        vec![ListItem::new(Span::styled(
            "  No posts yet. Press 'n' to compose.",
            Style::default().fg(DIM),
        ))]
    } else {
        app.timeline.iter().map(|msg| {
            let content = format!("[{}] {}", &msg.author[..12],
                match &msg.content {
                    crate::protocol::message::MessageContent::Post(p) => &p.text,
                    _ => "(other)",
                });
            ListItem::new(Span::raw(content))
        }).collect()
    };

    let list = List::new(items)
        .block(Block::default()
            .title(" Timeline ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)));

    frame.render_widget(list, area);
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
            Span::styled("Address: ", Style::default().fg(DIM)),
            Span::styled(&app.identity_address, Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::styled("Peers: ", Style::default().fg(DIM)),
            Span::raw(format!("{}", app.peer_count)),
        ]),
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
    };

    let input = Paragraph::new(content)
        .block(Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(match app.input_mode {
                InputMode::Normal => DIM,
                InputMode::Editing => ACCENT,
                InputMode::Command => Color::Yellow,
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

fn tab_span(label: &str, active: bool) -> Span {
    if active {
        Span::styled(label, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(label, Style::default().fg(DIM))
    }
}
