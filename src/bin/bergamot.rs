use bergamot::{
    create_output_windows, error::Error, get_connection, get_rectangles, get_screen, Colour,
    Command, Context, Cursors, Draw, Layout, Paint, Update, Widget,
};
use std::sync::{mpsc::channel, Arc, Mutex};

struct Config {
    height: u32,
    font_str: String,
    default_bg: Colour,
    default_fg: Colour,
}

fn display(context: &Context<Config>, widgets: &[Widget]) -> Vec<Paint> {
    let mut area_paints = vec![];

    for output in &context.outputs {
        let (centered, uncentered): (Vec<(&Widget, Layout)>, Vec<(&Widget, Layout)>) = widgets
            .iter()
            .map(|w| (w, Layout::new(&output.ctx, &w.area, &context.font.0)))
            .partition(|(w, _)| w.alignment.is_center());

        let center_width: f64 = centered.iter().map(|(_, l)| l.width).sum();

        let mut cursors = Cursors {
            top: 0.0,
            bottom: context.config.height.into(),
            left: 0.0,
            center: (output.rect.width / 2.0) - (center_width / 2.0),
            right: output.rect.width.into(),
        };

        output.ctx.set_colour(&context.config.default_bg);
        output.ctx.rectangle(&cursors.as_rectangle());
        output.ctx.fill();

        for (widget, layout) in uncentered.iter().chain(centered.iter()) {
            let bg = widget.area.colours.bg.unwrap_or(context.config.default_bg);
            let fg = widget.area.colours.fg.unwrap_or(context.config.default_fg);

            output.ctx.set_colour(&bg);

            let rect = cursors.make_bounding_rectangle(widget, layout);

            output.ctx.rectangle(&rect);
            output.ctx.fill();

            output.ctx.set_colour(&fg);

            output
                .ctx
                .move_to(rect.x + 5.0, rect.height / 2.0 - layout.height / 2.0);

            layout.display(&output.ctx);

            area_paints.push(Paint {
                left: rect.x,
                right: rect.x + rect.width,
                win: output.win,
                tag: widget.tag.clone(),
            });
        }
    }
    area_paints
}

fn main() -> Result<(), Error> {
    let cfg = Config {
        height: 16,
        font_str: "monospace 9".to_string(),
        default_bg: Colour {
            red: 0,
            green: 0,
            blue: 0,
        },
        default_fg: Colour {
            red: 255,
            green: 255,
            blue: 255,
        },
    };

    let font = bergamot::FontDescription::new(&cfg.font_str);

    let conn = get_connection()?;
    let screen = get_screen(&conn);
    let rectangles = get_rectangles(&conn, &screen)?;
    let windows = create_output_windows(&conn, &screen, cfg.height as i32, rectangles);

    let ctx = Context {
        config: cfg,
        outputs: windows,
        font,
    };

    conn.0.flush();

    let (tx, rx) = channel();

    let conn = Arc::new(conn);
    let paints = Arc::new(Mutex::new(Vec::new()));

    let widgets: Arc<Mutex<Vec<Widget>>> = Arc::new(Mutex::new(Vec::new()));

    let _stdin_handle = {
        let widgets = Arc::clone(&widgets);
        let tx = tx.clone();
        std::thread::spawn(move || {
            use std::io::BufRead;

            let stdin = std::io::stdin();
            let mut stdin = stdin.lock();

            let mut buf = String::new();

            loop {
                match stdin.read_line(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(command) = serde_json::from_str(&buf) {
                            match command {
                                Command::Update(Update { tag, area }) => {
                                    let mut widgets = widgets.lock().unwrap();
                                    let widget = widgets.iter_mut().find(|w| w.tag == tag);
                                    if let Some(mut widget) = widget {
                                        widget.area = area;
                                        tx.send(()).unwrap();
                                    } else {
                                        eprintln!("No such widget '{}'", tag);
                                    }
                                }
                                Command::Draw(Draw {
                                    widgets: new_widgets,
                                }) => {
                                    let mut widgets = widgets.lock().unwrap();
                                    widgets.clear();
                                    *widgets = new_widgets;
                                    tx.send(()).unwrap();
                                }
                            }
                        } else {
                            eprintln!("Failed to read command");
                            let _: Command = serde_json::from_str(&buf).unwrap();
                        }
                        buf.clear();
                    }
                    _ => break,
                }
            }
        })
    };

    let _draw_handle = {
        let conn = Arc::clone(&conn);
        let paints = Arc::clone(&paints);

        let widgets = Arc::clone(&widgets);
        std::thread::spawn(move || {
            while let Ok(_) = rx.recv() {
                let widgets = widgets.lock().unwrap();
                let new_paints = display(&ctx, &widgets);
                conn.flush();
                let mut paints = paints.lock().unwrap();
                let _ = std::mem::replace(&mut *paints, new_paints);
            }
        })
    };

    while let Some(event) = conn.0.wait_for_event() {
        match event.response_type() & !0x80 {
            xcb::EXPOSE => {
                tx.send(()).unwrap();
            }
            xcb::BUTTON_PRESS => {
                let event: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(&event) };
                let win = event.event();
                let x = event.event_x().into();

                let paints = paints.lock().unwrap();

                let paint = paints
                    .iter()
                    .filter(|p: &&Paint| p.win == win && p.left <= x && p.right >= x)
                    .min_by(|p1, p2| {
                        (p1.right - p1.left)
                            .partial_cmp(&(p2.right - p2.left))
                            .unwrap()
                    });

                if let Some(p) = paint {
                    let widgets = widgets.lock().unwrap();
                    let widget = widgets.iter().find(|w| w.tag == p.tag);

                    use bergamot::MouseButton;

                    let button = match event.detail() {
                        1 => Some(MouseButton::Left),
                        2 => Some(MouseButton::Middle),
                        3 => Some(MouseButton::Right),
                        _ => None,
                    };

                    if let (Some(button), Some(widget)) = (button, widget) {
                        let handlers = widget.area.on_click.iter().filter(|h| h.button == button);

                        for handler in handlers {
                            println!("{}", handler.output);
                        }
                    }
                }
            }
            _ => (),
        }
        conn.0.flush();
    }

    Ok(())
}
