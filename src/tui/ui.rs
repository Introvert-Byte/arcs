use super::app::{App, Screen};
use crate::util::human_size;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.size());

    draw_header(f, chunks[0], app);
    match app.screen {
        Screen::Home => draw_home(f, chunks[1]),
        Screen::Discover => draw_discover(f, chunks[1], app),
        Screen::BackupProgress => draw_progress(f, chunks[1], app),
        Screen::Inspector => draw_inspector(f, chunks[1], app),
        Screen::Restore => draw_restore(f, chunks[1], app),
        Screen::Help => draw_help(f, chunks[1]),
    }
    draw_status(f, chunks[2], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let title = match app.screen {
        Screen::Home => "Arcs – Backup & Migration",
        Screen::Discover => "Select Items to Backup",
        Screen::BackupProgress => "Backup Progress",
        Screen::Inspector => "Archive Inspector",
        Screen::Restore => "Restore",
        Screen::Help => "Help",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let p = Paragraph::new("  Cross-platform · Encrypted · Selective")
        .style(Style::default().fg(Color::DarkGray))
        .block(block);
    f.render_widget(p, area);
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let help = match app.screen {
        Screen::Home => "d/Enter: Discover  i: Inspect  r: Restore  ?: Help  q: Quit",
        Screen::Discover => "↑↓/jk: Navigate  Space: Toggle  a: All  n: None  Enter: Backup  Esc: Back",
        _ => "Esc/q: Back to Home",
    };
    let text = format!("{}  │  {}", app.status, help);
    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn draw_home(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  █████╗ ██████╗  ██████╗███████╗",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " ██╔══██╗██╔══██╗██╔════╝██╔════╝",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " ███████║██████╔╝██║     ███████╗",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " ██╔══██║██╔══██╗██║     ╚════██║",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " ██║  ██║██║  ██║╚██████╗███████║",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚══════╝",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from("  Secure, selective backups of apps, configs and data."),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Press "),
            Span::styled("d", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" to discover installed applications and configs."),
        ]),
    ];
    let p = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_discover(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|item| {
            let mark = if item.selected { "[x]" } else { "[ ]" };
            let kind = format!("{:?}", item.kind).to_lowercase();
            let size = item
                .size_hint
                .map(human_size)
                .unwrap_or_else(|| "—".into());
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", mark),
                    Style::default().fg(if item.selected {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(format!("{:<28}", item.name)),
                Span::styled(format!("{:<12}", kind), Style::default().fg(Color::Blue)),
                Span::raw(size),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Items ({} selected)",
            app.selected_count()
        )))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_progress(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent((app.progress * 100.0) as u16)
        .label(format!("{:.0}%", app.progress * 100.0));
    f.render_widget(gauge, chunks[0]);

    let info = Paragraph::new(vec![
        Line::from(format!("Output : {}", app.output_path)),
        Line::from(format!("Selected items : {}", app.selected_count())),
        Line::from(format!("Dry-run : {}", app.dry_run)),
        Line::from(""),
        Line::from("In a full implementation the backup would run here"),
        Line::from("with live progress updates and encryption."),
    ])
    .block(Block::default().borders(Borders::ALL).title("Details"));
    f.render_widget(info, chunks[1]);
}

fn draw_inspector(f: &mut Frame, area: Rect, _app: &App) {
    let p = Paragraph::new(
        "Archive Inspector\n\n\
         Use the CLI for now:\n\
         arcs inspect <file.arcs>\n\
         arcs search <file.arcs> <query>\n\
         arcs diff <a.arcs> <b.arcs>",
    )
    .block(Block::default().borders(Borders::ALL).title("Inspector"));
    f.render_widget(p, area);
}

fn draw_restore(f: &mut Frame, area: Rect, _app: &App) {
    let p = Paragraph::new(
        "Restore\n\n\
         CLI:\n\
         arcs restore <file.arcs> [--dry-run] [--password ...]\n\
         arcs restore <file.arcs> --target ~/restored",
    )
    .block(Block::default().borders(Borders::ALL).title("Restore"));
    f.render_widget(p, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let text = "\
Keyboard shortcuts

  Home
    d / Enter   Open discovery & selection
    i           Archive inspector
    r           Restore screen
    ?           This help
    q           Quit

  Discovery
    ↑ ↓ / j k   Move selection
    Space       Toggle item
    a           Select all
    n           Select none
    Enter       Start backup of selected items
    Esc         Back

  General
    Esc / q     Return to previous screen / quit
";
    let p = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}