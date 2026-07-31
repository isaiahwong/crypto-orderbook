use crossterm::{
    ExecutableCommand,
    event::{Event, EventStream, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{
    Terminal,
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::io::{self, stdout};
use tokio::sync::mpsc::Receiver;

use crate::{
    candle::{CandleChart, GREEN},
    types::{Candles, Message, Orderbook},
};

#[derive(Default)]
pub struct State {
    pub orderbook: Orderbook,
    pub candles: Candles,
    pub mouse_event: Option<crossterm::event::MouseEvent>,
}

pub struct App {
    rx: Receiver<Result<Message, anyhow::Error>>,
    symbol: String,
    quit: bool,
    state: State,
}

impl App {
    pub fn new(symbol: impl Into<String>, rx: Receiver<Result<Message, anyhow::Error>>) -> anyhow::Result<Self> {
        Ok(Self {
            rx,
            symbol: symbol.into(),
            quit: false,
            state: State::default(),
        })
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        enable_raw_mode()?;
        terminal.clear()?;
        stdout().execute(EnterAlternateScreen)?;
        stdout().execute(crossterm::event::EnableMouseCapture)?;

        let run_result = self.draw_loop(&mut terminal).await;

        stdout().execute(crossterm::event::DisableMouseCapture)?;
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        run_result
    }

    async fn draw_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
        let mut events = EventStream::new();

        while !self.quit {
            tokio::select! {
                Some(msg) = self.rx.recv() => {
                    self.on_message(msg);
                }
                Some(Ok(event)) = events.next() => {
                    self.on_events(event);
                }
            }

            self.draw(terminal)?;
        }

        Ok(())
    }

    fn on_events(&mut self, event: Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                _ => {}
            },
            Event::Mouse(mouse_event) => {
                self.state.mouse_event = Some(mouse_event);
            }
            _ => {}
        }
    }

    fn on_message(&mut self, msg: anyhow::Result<Message>) {
        let Ok(message) = msg else { return };
        match message {
            Message::BookSnapshot(depth) => {
                self.state.orderbook.apply_depth(depth);
            }
            Message::CandleSnapshot(klines) => {
                self.state.candles = klines.into();
            }
            Message::Candle(candle) => {
                self.state.candles.upsert(candle);
            }
        }
    }

    fn draw(&self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
        terminal.draw(|f| self.render(f))?;
        Ok(())
    }

    fn render(&self, f: &mut Frame) {
        let layout = Layout::default()
            .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
            .split(f.area());

        self.render_header(f, layout[0]);
        self.render_content(f, layout[1]);
        self.render_footer(f, layout[2]);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let price = self.state.candles.last_price();
        let pct = self.state.candles.pct_change();

        let pct_color = if pct > 0.0 {
            GREEN
        } else if pct < 0.0 {
            Color::Red
        } else {
            Color::White
        };

        let line = Line::from(vec![
            Span::raw(" Last: "),
            Span::styled(format!("{price:.2}"), Style::default().fg(Color::Yellow)),
            Span::raw("   "),
            Span::styled(format!("{pct:+.2}%"), Style::default().fg(pct_color)),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                self.symbol.to_uppercase(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ));

        f.render_widget(Paragraph::new(line).block(block), area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let key = Style::default().fg(Color::Black).bg(Color::Gray);
        let label = Style::default().fg(Color::Gray);

        let line = Line::from(vec![Span::raw(" "), Span::styled(" q ", key), Span::styled(" quit", label)]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        f.render_widget(Paragraph::new(line).block(block), area);
    }

    fn render_content(&self, f: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        f.render_widget(CandleChart::new(&self.state.candles, self.state.mouse_event), cols[0]);
        f.render_widget(crate::dom::DomWidget::new(&self.state.orderbook), cols[1]);
    }
}
