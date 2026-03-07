//! TUI rendering logic.

use crate::app::{App, Focus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let main_chunks = if app.file_browser.visible {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30), // File browser
                Constraint::Min(40),    // Main content
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40)])
            .split(f.area())
    };

    let content_area = if app.file_browser.visible {
        draw_file_browser(f, app, main_chunks[0]);
        main_chunks[1]
    } else {
        main_chunks[0]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Length(3), // Input
            Constraint::Length(4), // Explainer
            Constraint::Min(5),    // Results
            Constraint::Length(1), // Status bar
        ])
        .split(content_area);

    draw_tabs(f, app, chunks[0]);
    draw_input(f, app, chunks[1]);
    draw_explainer(f, app, chunks[2]);
    draw_results(f, app, chunks[3]);
    draw_status(f, app, chunks[4]);

    if app.show_help {
        draw_help(f);
    }
}

fn draw_file_browser(f: &mut Frame, app: &App, area: Rect) {
    let items = app.file_browser.visible_items();
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (depth, node))| {
            let indent = "  ".repeat(*depth);
            let icon = if node.is_dir {
                if node.expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            let style = if i == app.file_browser.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if node.is_dir {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{indent}{icon}{}", node.name)).style(style)
        })
        .collect();

    let border_color = if app.focus == Focus::FileBrowser {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let browser = List::new(list_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Files ")
            .border_style(Style::default().fg(border_color)),
    );
    f.render_widget(browser, area);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = [
        "Search",
        "Transform",
        "Analyze",
        "Pipeline",
        "AQL",
        "Symbols",
    ]
    .iter()
    .map(|t| Line::from(*t))
    .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ATP — Agentic Text Processor "),
        )
        .select(app.tab_index())
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let input_text = if let Some(ref suggestion) = app.suggestion {
        let typed = &app.input;
        let rest = &suggestion[typed.len()..];
        vec![
            Span::styled(typed.clone(), Style::default().fg(Color::White)),
            Span::styled(rest.to_string(), Style::default().fg(Color::DarkGray)),
        ]
    } else {
        vec![Span::styled(
            app.input.clone(),
            Style::default().fg(Color::White),
        )]
    };

    let label = match app.active_tab {
        crate::app::Tab::Search => " Pattern (regex) ",
        crate::app::Tab::Transform => " Expression (s/pattern/replacement/flags) ",
        crate::app::Tab::Analyze => " Configuration ",
        crate::app::Tab::Pipeline => " Pipeline DSL ",
        crate::app::Tab::Aql => " AQL Query ",
        crate::app::Tab::Symbols => " Symbol Filter (kind:pattern) ",
        crate::app::Tab::Index => " Index Filter (glob/substring) ",
        crate::app::Tab::Debug => " AQL Debug Query ",
        crate::app::Tab::Notebook => " Notebook Path (.atp.md) ",
        crate::app::Tab::Distributed => " Distributed DSL @nodes ",
    };

    let input = Paragraph::new(Line::from(input_text)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(label)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(input, area);

    // Show cursor
    f.set_cursor_position((area.x + app.cursor_pos as u16 + 1, area.y + 1));
}

fn draw_explainer(f: &mut Frame, app: &App, area: Rect) {
    let explainer = Paragraph::new(app.explainer_text.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Explainer ")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });
    f.render_widget(explainer, area);
}

fn draw_results(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let style = if i == app.selected_result {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let highlight = m.line_content.clone(); // ratatui doesn't support inline color changes in ListItem easily

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{}:{}:{} ", m.file, m.line_number, m.column_start),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(highlight, style),
            ]))
        })
        .collect();

    let title = format!(" Results ({}) — ↑↓ to navigate ", app.results.len());
    let results = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Blue)),
    );
    f.render_widget(results, area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let status = Paragraph::new(Line::from(vec![
        Span::styled(" ATP ", Style::default().fg(Color::Black).bg(Color::Cyan)),
        Span::raw(" "),
        Span::styled(&app.status_message, Style::default().fg(Color::Gray)),
    ]));
    f.render_widget(status, area);
}

fn draw_help(f: &mut Frame) {
    let area = centered_rect(60, 60, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        "",
        "  ATP — Agentic Text Processor — Interactive Mode",
        "",
        "  Navigation:",
        "    Tab / Shift+Tab   Switch between modes",
        "    ↑ / ↓             Navigate results",
        "    PgUp / PgDn       Page through results",
        "    →                 Accept suggestion",
        "    Enter             Execute current command",
        "    Ctrl+F            Cycle focus (Input→Results→Files)",
        "",
        "  Modes:",
        "    Search            Live regex search across files",
        "    Transform         Sed-style text transformation",
        "    Analyze           Awk-style field processing",
        "    Pipeline          Multi-stage pipeline builder",
        "    AQL               ATP Query Language editor",
        "    Symbols           Code intelligence & symbol search",
        "",
        "  File Browser:",
        "    Ctrl+B            Toggle file browser panel",
        "    Enter             Expand dir / preview file",
        "",
        "  Controls:",
        "    F1                Toggle this help",
        "    Ctrl+Q / Esc      Quit",
        "",
    ];

    let help = Paragraph::new(help_text.join("\n"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help (F1 to close) ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });
    f.render_widget(help, area);
}

/// Create a centered rect of given percentage dimensions.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
