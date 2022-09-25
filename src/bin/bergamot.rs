use bergamot::{
    create_output_windows, error::Error, get_connection, get_rectangles, get_screen, Area, Colour,
    Command, Output, Config, Cursors, Draw, Layout, Paint, Update, Widget,
};
use std::sync::{mpsc::channel, Arc, Mutex};

fn display(windows: &[Output], widgets: &[Widget]) -> Vec<Paint> {
    let mut area_paints = vec![];

    for (output_no, output) in windows.iter().enumerate() {
        let (centered, mut uncentered): (Vec<(&Widget, &Area, Layout)>, _) =
            widgets
            .iter()
            .flat_map(|w| {
                w.content
                    .iter()
                    .map(move |a| (w, a, Layout::new(&output.ctx, a, &output.font.0)))
            })
            .partition(|(w, _, _)| w.alignment.is_center());
	
	let (right, left): (Vec<(&Widget, &Area, Layout)>, _) = uncentered
	    .drain(..)
	    .partition(|(w, _, _)| w.alignment.is_right());
	
        let center_width: f64 = centered.iter().map(|(_, _, l)| l.width).sum();
	let right_width: f64 = right.iter().map(|(_, _, l)| l.width).sum();

        let mut cursors = Cursors {
            top: 0.0,
            bottom: output.cfg.height as f64,
            left: 0.0,
            center: (output.rect.width / 2.0) - (center_width / 2.0),
            right: output.rect.width - right_width,
        };

        output.ctx.set_colour(&output.cfg.default_bg);
        output.ctx.rectangle(&cursors.as_rectangle());
        output.ctx.fill();

        for (widget, area, layout) in left.iter().chain(right.iter()).chain(centered.iter()) {
            let monitor_constaints: Vec<_> = widget.constraints.monitor().collect();

            if !monitor_constaints.is_empty()
                && !monitor_constaints.iter().any(|m| m.number() == output_no)
            {
                continue;
            }

            let bg = area.colours.bg.unwrap_or(output.cfg.default_bg);
            let fg = area.colours.fg.unwrap_or(output.cfg.default_fg);

            output.ctx.set_colour(&bg);

            let rect = cursors.make_bounding_rectangle(widget, layout);

            output.ctx.rectangle(&rect);
            output.ctx.fill();

	    output.ctx.status();

            output.ctx.set_colour(&fg);

            output
                .ctx
                .move_to(rect.x + 5.0, rect.height / 2.0 - layout.height / 2.0);

            layout.display(&output.ctx);

	    output.ctx.status();

            area_paints.push(Paint {
                left: rect.x,
                right: rect.x + rect.width,
                win: output.win,
                area: (*area).clone(),
            });
        }
    }
    
    area_paints
}

fn main() -> Result<(), Error> {
    use std::str::FromStr;
    
    let cfgs = vec![
	Config {
            height: 14,
            font_str: "Iosevka Term 9".to_string(),
            default_bg: Colour::from_str("#333232").unwrap(),
            default_fg: Colour::from_str("#a7a5a5").unwrap()
	},
	Config {
            height: 18,
            font_str: "Iosevka Term 12".to_string(),
            default_bg: Colour::from_str("#333232").unwrap(),
            default_fg: Colour::from_str("#a7a5a5").unwrap()
	},
	Config {
            height: 18,
            font_str: "Iosevka Term 12".to_string(),
            default_bg: Colour::from_str("#333232").unwrap(),
            default_fg: Colour::from_str("#a7a5a5").unwrap()
	},
    ];
	
    let conn = get_connection()?;
    let screen = get_screen(&conn);
    let rectangles = get_rectangles(&conn, &screen)?;
    let windows = create_output_windows(&conn, &screen, &cfgs, rectangles);


    conn.0.flush().expect("Failed to flush connection");

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
            let stdin = stdin.lock();

            for line in stdin.lines() {
                if let Ok(line) = line {
                    match serde_json::from_str(&line) {
			Ok(command) => 
                            match command {
				Command::Update(Update { tag, content }) => {
                                    if tag == "" {
					eprintln!("Cannot update an untagged widget");
					continue;
                                    }
                                    let mut widgets = widgets.lock().unwrap();
                                    let widget = widgets.iter_mut().find(|w| w.tag == tag);
                                    if let Some(mut widget) = widget {
					widget.content = content;
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
                            },
			Err(e) => {
			    eprintln!("Failed to read command at line <{}>\nError: {}", line, e);
			}
		    }
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
                let new_paints = display(&windows, &widgets);
                conn.flush();
                let mut paints = paints.lock().unwrap();
                let _ = std::mem::replace(&mut *paints, new_paints);
            }
        })
    };

    while let Ok(xcb::Event::X(event)) = conn.0.wait_for_event() {
	match event {
	    xcb::x::Event::Expose(_) => {
		tx.send(()).unwrap();
	    },
	    xcb::x::Event::ButtonPress(evt) => {
                let win = evt.event();
                let x = evt.event_x().into();

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
                    use bergamot::MouseButton;

                    let button = match evt.detail() {
                        1 => Some(MouseButton::Left),
                        2 => Some(MouseButton::Middle),
                        3 => Some(MouseButton::Right),
                        4 => Some(MouseButton::ScrollUp),
                        5 => Some(MouseButton::ScrollDown),
                        6 => Some(MouseButton::ScrollLeft),
                        7 => Some(MouseButton::ScrollRight),
                        _ => None,
                    };

                    if let Some(button) = button {
                        let handlers = p.area.on_click.iter().filter(|h| h.button == button);

                        for handler in handlers {
                            println!("{}", handler.output);
                        }
                    }
                }
	    },
	    _ => {}
	}
        conn.0.flush().expect("Failed to flush connection");
    }
    Ok(())
}
