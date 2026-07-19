use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::app::{App, InputMode, View};
use crate::protocol::message::MessageContent;

const ACCENT: Color = Color::Rgb(224, 122, 95);
const REPLY_COLOR: Color = Color::Rgb(107, 155, 210);
const DIM: Color = Color::DarkGray;
const BOOKMARK_COLOR: Color = Color::Rgb(233, 196, 106);
const COMMAND_COLOR: Color = Color::Rgb(233, 196, 106);
const REPLY_MODE_COLOR: Color = Color::Rgb(192, 139, 210);
const BORDER_DIM: Color = Color::Rgb(58, 58, 58);
const TIME_COLOR: Color = Color::Rgb(85, 85, 85);

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
    draw_footer(frame, app, chunks[3]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let tabs: Vec<Span> = vec![
        tab_span("t:Timeline", app.view == View::Timeline),
        Span::raw(" | "),
        tab_span("d:DMs", app.view == View::DirectMessages),
        Span::raw(" | "),
        tab_span("c:Communities", app.view == View::Communities),
        Span::raw(" | "),
        tab_span("b:Bookmarks", app.view == View::Bookmarks),
        Span::raw(" | "),
        tab_span("p:Profile", app.view == View::Profile),
    ];

    let (status_icon, status_color) = if app.onion_address.is_some() {
        ("●", ACCENT)
    } else {
        ("○", COMMAND_COLOR)
    };

    let status_label = if app.onion_address.is_some() {
        "online"
    } else {
        "connecting"
    };
    let status_text = format!(
        "{} {} | {} peers ",
        status_icon, status_label, app.peer_count
    );

    let header = Paragraph::new(Line::from(tabs)).block(
        Block::default()
            .title(" Y ")
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT)),
    );

    frame.render_widget(header, area);

    let status_width = status_text.len() as u16;
    if area.width > status_width + 2 {
        let status_area = Rect::new(
            area.x + area.width - status_width - 2,
            area.y + 1,
            status_width,
            1,
        );
        let status = Paragraph::new(Span::styled(status_text, Style::default().fg(status_color)));
        frame.render_widget(status, status_area);
    }
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    match app.view {
        View::Timeline => draw_post_list(frame, app, &app.timeline, " Timeline ", area),
        View::DirectMessages => draw_dms(frame, area),
        View::Communities => draw_communities(frame, app, area),
        View::Profile => draw_profile(frame, app, area),
        View::Compose => draw_compose(frame, app, area),
        View::Search => draw_search(frame, app, area),
        View::Bookmarks => draw_post_list(frame, app, &app.bookmarks, " Bookmarks ", area),
        View::Thread => draw_post_list(frame, app, &app.timeline, " Thread ", area),
        View::CommunityDetail => draw_community_detail(frame, app, area),
    }
}

fn format_relative_time(timestamp: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(*timestamp);

    if diff.num_seconds() < 60 {
        "now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h", diff.num_hours())
    } else if diff.num_days() < 30 {
        format!("{}d", diff.num_days())
    } else {
        format!("{}mo", diff.num_days() / 30)
    }
}

fn draw_post_list(
    frame: &mut Frame,
    app: &App,
    _posts: &[crate::protocol::message::Message],
    title: &str,
    area: Rect,
) {
    let entries = app.visible_entries();

    let items: Vec<ListItem> = if entries.is_empty() {
        let empty_height = area.height.saturating_sub(4) as usize;
        let pad_top = empty_height / 2;
        let mut lines: Vec<ListItem> = Vec::new();
        for _ in 0..pad_top.saturating_sub(2) {
            lines.push(ListItem::new(Span::raw("")));
        }

        let (icon, title_text, hint, action) = if app.view == View::Bookmarks {
            (
                "★",
                "Nothing saved yet",
                "Bookmarks are local. Only you can see them.",
                "Press [s] on any post to save it here",
            )
        } else {
            (
                "◇",
                "No posts yet",
                "Be the first to say something.",
                "Press [n] to compose a post",
            )
        };
        let icon_color = if app.view == View::Bookmarks {
            BOOKMARK_COLOR
        } else {
            BORDER_DIM
        };

        lines.push(ListItem::new(Line::from(vec![Span::styled(
            format!("{:^width$}", icon, width = area.width as usize - 2),
            Style::default().fg(icon_color),
        )])));
        lines.push(ListItem::new(Line::from(vec![Span::styled(
            format!("{:^width$}", title_text, width = area.width as usize - 2),
            Style::default().fg(DIM),
        )])));
        lines.push(ListItem::new(Line::from(vec![Span::styled(
            format!("{:^width$}", hint, width = area.width as usize - 2),
            Style::default().fg(TIME_COLOR),
        )])));
        lines.push(ListItem::new(Line::from(vec![Span::styled(
            format!("{:^width$}", action, width = area.width as usize - 2),
            Style::default().fg(ACCENT),
        )])));
        lines
    } else {
        entries
            .iter()
            .enumerate()
            .flat_map(|(i, entry)| {
                let is_selected = i == app.selected_post;
                let depth = entry.depth;

                let mut tree_prefix = String::new();
                for d in 0..depth {
                    if d < entry.ancestors_continuing.len() && entry.ancestors_continuing[d] {
                        tree_prefix.push_str("│  ");
                    } else {
                        tree_prefix.push_str("   ");
                    }
                }

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

                let time_str = format_relative_time(&msg.timestamp);

                let is_reply = depth > 0;
                let base_color = if is_reply { REPLY_COLOR } else { ACCENT };

                let header_line = Line::from(vec![
                    Span::styled(
                        format!("  {}{}", tree_prefix, connector),
                        Style::default().fg(BORDER_DIM),
                    ),
                    Span::styled(
                        author_display,
                        if is_selected {
                            Style::default().fg(base_color).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(base_color)
                        },
                    ),
                    Span::styled(format!(" {}", time_str), Style::default().fg(TIME_COLOR)),
                ]);

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

                let content_line = format!("{}{}", continuation, text);
                let content_style = if is_reply {
                    Style::default().fg(REPLY_COLOR)
                } else {
                    Style::default()
                };

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

                let mut meta_spans = vec![
                    Span::styled(
                        format!("{}{}", continuation, nods),
                        Style::default().fg(DIM),
                    ),
                    Span::styled(" · ", Style::default().fg(BORDER_DIM)),
                    Span::styled(replies, Style::default().fg(DIM)),
                ];
                if is_bookmarked {
                    meta_spans.push(Span::styled(" ★", Style::default().fg(BOOKMARK_COLOR)));
                }

                let selected_bg = if is_selected {
                    Style::default().bg(Color::Rgb(40, 28, 24))
                } else {
                    Style::default()
                };

                let mut lines = vec![
                    ListItem::new(header_line).style(selected_bg),
                    ListItem::new(Span::styled(content_line, content_style)).style(selected_bg),
                    ListItem::new(Line::from(meta_spans)).style(selected_bg),
                ];

                let has_replies = !entry.is_collapse_marker && msg.reply_count() > 0;
                if has_replies && depth == 0 {
                    lines.push(ListItem::new(Span::styled(
                        "  │",
                        Style::default().fg(BORDER_DIM),
                    )));
                } else if depth == 0 {
                    lines.push(ListItem::new(Span::styled(
                        format!("{:─>width$}", "", width = area.width as usize - 4),
                        Style::default().fg(Color::Rgb(42, 42, 42)),
                    )));
                } else {
                    lines.push(ListItem::new(Span::raw("")));
                }

                lines
            })
            .collect()
    };

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
    let list = List::new(items_to_show).block(
        Block::default()
            .title(title)
            .title_bottom(Line::from(Span::styled(help, Style::default().fg(DIM))))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM)),
    );

    frame.render_widget(list, area);
}

fn draw_dms(frame: &mut Frame, area: Rect) {
    let empty_height = area.height.saturating_sub(4) as usize;
    let pad_top = empty_height / 2;
    let mut lines: Vec<Line> = Vec::new();
    for _ in 0..pad_top.saturating_sub(2) {
        lines.push(Line::from(""));
    }
    let w = area.width as usize - 2;
    lines.push(Line::from(Span::styled(
        format!("{:^w$}", "◇"),
        Style::default().fg(BORDER_DIM),
    )));
    lines.push(Line::from(Span::styled(
        format!("{:^w$}", "No conversations yet"),
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(Span::styled(
        format!(
            "{:^w$}",
            "End-to-end encrypted. No one else can read these."
        ),
        Style::default().fg(TIME_COLOR),
    )));
    lines.push(Line::from(Span::styled(
        format!("{:^w$}", "Search for a user to start a DM  [/]"),
        Style::default().fg(ACCENT),
    )));

    let block = Paragraph::new(lines).block(
        Block::default()
            .title(" Direct messages ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM)),
    );
    frame.render_widget(block, area);
}

fn draw_communities(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    if app.communities.is_empty() {
        let empty_height = area.height.saturating_sub(4) as usize;
        let pad_top = empty_height / 2;
        for _ in 0..pad_top.saturating_sub(2) {
            lines.push(Line::from(""));
        }
        let w = area.width as usize - 2;
        lines.push(Line::from(Span::styled(
            format!("{:^w$}", "◈"),
            Style::default().fg(BORDER_DIM),
        )));
        lines.push(Line::from(Span::styled(
            format!("{:^w$}", "No communities joined"),
            Style::default().fg(DIM),
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "{:^w$}",
                "Communities are self-governed groups with shared timelines."
            ),
            Style::default().fg(TIME_COLOR),
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "{:^w$}",
                ":create <name> to start one  ·  :join <id> to join"
            ),
            Style::default().fg(ACCENT),
        )));
    } else {
        for (i, community) in app.communities.iter().enumerate() {
            let is_selected = i == app.selected_list_item;
            let member_count = community.members.len();
            let lock_icon = if community.is_locked { "◆" } else { "◇" };
            let is_owner = community.owner == app.identity_address;
            let role = if is_owner { "owner" } else { "member" };
            let pending = community.pending_requests.len();

            let bg = if is_selected {
                Style::default().bg(Color::Rgb(40, 28, 24))
            } else {
                Style::default()
            };

            let name_style = if is_selected {
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Rgb(40, 28, 24))
            } else {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            };

            let mut meta = format!("  {} members · {}", member_count, role);
            if pending > 0 && is_owner {
                meta.push_str(&format!(" · {} pending", pending));
            }

            lines.push(
                Line::from(vec![
                    Span::styled(format!(" {} ", lock_icon), Style::default().fg(BORDER_DIM)),
                    Span::styled(&community.name, name_style),
                    Span::styled(meta, Style::default().fg(DIM)),
                ])
                .style(bg),
            );
            lines.push(
                Line::from(Span::styled(
                    format!("     {}", community.id),
                    Style::default().fg(TIME_COLOR),
                ))
                .style(bg),
            );
            lines.push(Line::from(""));
        }
    }

    let block = Paragraph::new(lines)
        .scroll((app.scroll_offset as u16, 0))
        .block(
            Block::default()
                .title(" Communities ")
                .title_bottom(Line::from(Span::styled(
                    " :create <name>  :join <id> ",
                    Style::default().fg(DIM),
                )))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_DIM)),
        );
    frame.render_widget(block, area);
}

fn draw_community_detail(frame: &mut Frame, app: &App, area: Rect) {
    let community = match app.selected_community.and_then(|i| app.communities.get(i)) {
        Some(c) => c,
        None => return,
    };

    let is_owner = community.owner == app.identity_address;
    let lock_label = if community.is_locked {
        "private"
    } else {
        "open"
    };

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(
            &community.name,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} · {} members", lock_label, community.members.len()),
            Style::default().fg(DIM),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        &community.id,
        Style::default().fg(TIME_COLOR),
    )));
    lines.push(Line::from(""));

    let mut list_index: usize = 0;

    if is_owner && !community.pending_requests.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("  Pending requests ({})", community.pending_requests.len()),
            Style::default()
                .fg(BOOKMARK_COLOR)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        for req in &community.pending_requests {
            let is_selected = list_index == app.selected_list_item;
            let bg = if is_selected {
                Style::default().bg(Color::Rgb(40, 28, 24))
            } else {
                Style::default()
            };

            let addr_display = if req.len() > 24 {
                format!("{}...{}", &req[..12], &req[req.len() - 8..])
            } else {
                req.clone()
            };

            lines.push(
                Line::from(vec![
                    Span::styled(
                        if is_selected { " ▸ " } else { "   " },
                        Style::default().fg(ACCENT),
                    ),
                    Span::styled(
                        addr_display,
                        if is_selected {
                            Style::default().fg(Color::White).bg(Color::Rgb(40, 28, 24))
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    if is_selected {
                        Span::styled(
                            "   [a]=approve  [x]=decline",
                            Style::default().fg(DIM).bg(Color::Rgb(40, 28, 24)),
                        )
                    } else {
                        Span::raw("")
                    },
                ])
                .style(bg),
            );
            list_index += 1;
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        format!("  Members ({})", community.members.len()),
        Style::default()
            .fg(REPLY_COLOR)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let mut members: Vec<&String> = community.members.iter().collect();
    members.sort();
    for member in members {
        let is_selected = list_index == app.selected_list_item;
        let bg = if is_selected {
            Style::default().bg(Color::Rgb(40, 28, 24))
        } else {
            Style::default()
        };

        let addr_display = if member.len() > 24 {
            format!("{}...{}", &member[..12], &member[member.len() - 8..])
        } else {
            member.clone()
        };

        let role_tag = if *member == community.owner {
            Span::styled(" owner", Style::default().fg(BOOKMARK_COLOR))
        } else {
            Span::raw("")
        };

        lines.push(
            Line::from(vec![
                Span::styled(
                    if is_selected { " ▸ " } else { "   " },
                    Style::default().fg(ACCENT),
                ),
                Span::styled(
                    addr_display,
                    if is_selected {
                        Style::default().fg(Color::White).bg(Color::Rgb(40, 28, 24))
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                role_tag,
            ])
            .style(bg),
        );
        list_index += 1;
    }

    let block = Paragraph::new(lines)
        .scroll((app.scroll_offset as u16, 0))
        .block(
            Block::default()
                .title(format!(" {} ", community.name))
                .title_bottom(Line::from(Span::styled(
                    " [Esc]=back  [j/k]=navigate ",
                    Style::default().fg(DIM),
                )))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_DIM)),
        );
    frame.render_widget(block, area);
}

fn draw_profile(frame: &mut Frame, app: &App, area: Rect) {
    let info = vec![
        Line::from(vec![
            Span::styled("Handle:    ", Style::default().fg(DIM)),
            Span::styled(
                &app.handle,
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
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
            Span::styled("Onion:     ", Style::default().fg(DIM)),
            Span::styled(
                app.onion_address.as_deref().unwrap_or("bootstrapping..."),
                Style::default().fg(ACCENT),
            ),
            Span::styled("  [y]=copy", Style::default().fg(DIM)),
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
        .block(
            Block::default()
                .title(" Identity ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_DIM)),
        );
    frame.render_widget(block, area);
}

fn draw_compose(frame: &mut Frame, app: &App, area: Rect) {
    let title = " Compose (Esc=cancel, Enter=post, Shift+Enter=new line) ";
    let block = Paragraph::new(app.input_buffer.as_str()).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT)),
    );
    frame.render_widget(block, area);

    let text_before_cursor = &app.input_buffer[..app.cursor_pos];
    let line_num = text_before_cursor.matches('\n').count();
    let col = text_before_cursor
        .rfind('\n')
        .map(|pos| app.cursor_pos - pos - 1)
        .unwrap_or(app.cursor_pos);
    frame.set_cursor_position((area.x + 1 + col as u16, area.y + 1 + line_num as u16));
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let (title, content, cursor_offset) = match app.input_mode {
        InputMode::Normal => ("Normal", String::new(), 0),
        InputMode::Editing => ("Insert", app.input_buffer.clone(), app.cursor_pos),
        InputMode::Command => (
            "Command",
            format!(":{}", app.input_buffer),
            app.cursor_pos + 1,
        ),
        InputMode::SearchInput => (
            "Search",
            format!("/{}", app.input_buffer),
            app.cursor_pos + 1,
        ),
        InputMode::Replying => ("Reply", app.input_buffer.clone(), app.cursor_pos),
    };

    let border_color = match app.input_mode {
        InputMode::Normal => BORDER_DIM,
        InputMode::Editing => ACCENT,
        InputMode::Command => COMMAND_COLOR,
        InputMode::SearchInput => REPLY_COLOR,
        InputMode::Replying => REPLY_MODE_COLOR,
    };

    let input = Paragraph::new(content).block(
        Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color)),
    );

    frame.render_widget(input, area);

    if app.input_mode != InputMode::Normal && app.input_mode != InputMode::Editing {
        frame.set_cursor_position((area.x + 1 + cursor_offset as u16, area.y + 1));
    }
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let handle_span = Span::styled(&app.handle, Style::default().fg(ACCENT));
    let display_msg = truncate_onion(&app.status_message);

    let right_width = display_msg.len() as u16;
    let left_width = app.handle.len() as u16;
    let spacer_width = area.width.saturating_sub(left_width + right_width + 2) as usize;
    let spacer = " ".repeat(spacer_width);

    let footer = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        handle_span,
        Span::raw(spacer),
        Span::styled(display_msg, Style::default().fg(DIM)),
    ]));
    frame.render_widget(footer, area);
}

fn truncate_onion(msg: &str) -> String {
    if let Some(pos) = msg.find(".onion") {
        let before_onion = &msg[..pos];
        let addr_start = before_onion.rfind([' ', ':']).map(|i| i + 1).unwrap_or(0);
        let onion_addr = &msg[addr_start..];
        if onion_addr.len() > 20 {
            let prefix = &msg[..addr_start];
            let short = format!(
                "{}...{}",
                &onion_addr[..8],
                &onion_addr[onion_addr.len() - 12..]
            );
            format!("{}{}", prefix, short)
        } else {
            msg.to_string()
        }
    } else {
        msg.to_string()
    }
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
        let hint = if app.input_buffer.is_empty() {
            "  Start typing to find users by alias or address"
        } else {
            "  No users found"
        };
        lines.push(Line::from(Span::styled(hint, Style::default().fg(DIM))));
    } else {
        for result in &app.search_results {
            lines.push(Line::from(Span::styled(
                result,
                Style::default().fg(ACCENT),
            )));
        }
    }

    let block = Paragraph::new(lines).block(
        Block::default()
            .title(" Search Users [/] ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT)),
    );
    frame.render_widget(block, area);
}

fn tab_span(label: &str, active: bool) -> Span<'_> {
    if active {
        Span::styled(
            label,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(label, Style::default().fg(DIM))
    }
}
