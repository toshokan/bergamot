use bergamot::{create_output_windows, get_connection, get_rectangles, get_screen};
use bergamot::{error::Error, Align, Area, Colour, Output, Paint};
use std::sync::{mpsc::channel, Arc, Mutex};

struct Config {
    height: u32,
    font_str: String,
    bg: Colour,
}

fn get_layout() -> Option<Vec<Area>> {
    let areas = vec![
        Area {
            align: Align::Left,
            tag: "left".to_string(),
            text: "clicky_left".to_string(),
            fg: Colour {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
            },
            bg: None,
            onclick: Some("left".to_string()),
        },
        Area {
            align: Align::Right,
            tag: "left".to_string(),
            text: "clicky_right".to_string(),
            fg: Colour {
                red: 0.0,
                green: 0.0,
                blue: 1.0,
            },
            bg: None,
            onclick: Some("right".to_string()),
        },
    ];
    Some(areas)
}

fn display(cfg: &Config, outputs: &[Output], areas: &[Area]) -> Vec<Paint> {
    let mut area_paints = vec![];
    let font = pango::FontDescription::from_string(&cfg.font_str);

    for output in outputs {
        output
            .ctx
            .set_source_rgb(cfg.bg.red, cfg.bg.blue, cfg.bg.green);
        output.ctx.rectangle(
            0.0,
            0.0,
            output.rect.width.into(),
            output.rect.height.into(),
        );
        output.ctx.fill();

        let mut left_finger = 0_f64;
        let mut right_finger = output.rect.width.into();
	
        for area in areas {
            let layout = pangocairo::create_layout(&output.ctx)
                .expect("Failed to create pangocairo layout");
            layout.set_font_description(Some(&font));
            layout.set_text(&area.text);

            let (w, h) = layout.get_pixel_size();
            let area_width: f64 = (w + 10).into();
            let layout_height: f64 = h.into();

            let bg = area.bg.as_ref().unwrap_or(&Colour {
                red: 0_f64,
                blue: 0_f64,
                green: 0_f64,
            });

            output.ctx.set_source_rgb(bg.red, bg.green, bg.blue);
            let (left, right) = match area.align {
                Align::Left => {
                    let (left, right) = (left_finger, left_finger + area_width);
                    left_finger += area_width;
                    (left, right)
                }
                Align::Right => {
                    let (left, right) = (right_finger - area_width, right_finger);
                    right_finger -= area_width;
                    (left, right)
                }
            };

            let height = cfg.height.into();

            output.ctx.rectangle(left, 0_f64, right - left, height);
            output.ctx.fill();

            output
                .ctx
                .set_source_rgb(area.fg.red, area.fg.green, area.fg.blue);
            output
                .ctx
                .move_to(left + 5_f64, height / 2_f64 - layout_height / 2_f64);
            pangocairo::show_layout(&output.ctx, &layout);

            area_paints.push(Paint { left, right, win: output.win, area: area.clone() })
        }
    }
    area_paints
}

fn main() -> Result<(), Error> {
    let cfg = Config {
        height: 16,
        font_str: "monospace 9".to_string(),
        bg: Colour {
            red: 0.1,
            blue: 0.1,
            green: 0.1,
        },
    };

    let conn = get_connection()?;
    let screen = get_screen(&conn);
    let rectangles = get_rectangles(&conn, &screen)?;
    let windows = create_output_windows(&conn, &screen, cfg.height as i32, rectangles);

    conn.0.flush();

    let (tx, rx) = channel();

    let conn = Arc::new(conn);
    let layout = Arc::new(Mutex::new(get_layout().unwrap()));
    let paints = Arc::new(Mutex::new(Vec::new()));

    let _stdin_handle = {
	let layout = Arc::clone(&layout);
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
			dbg!(&buf);
			let new_layout = get_layout().unwrap();
			let mut layout = layout.lock().unwrap();
			let _ = std::mem::replace(&mut *layout, new_layout);
			buf.clear();

			tx.send(()).unwrap();
		    },
		    _ => break,
		}
	    }
	})
    };

    let _draw_handle = {
	let conn = Arc::clone(&conn);
	let layout = Arc::clone(&layout);
	let paints = Arc::clone(&paints);
	std::thread::spawn(move || {
	    while let Ok(_) = rx.recv() {
		let layout = layout.lock().unwrap();
		let new_paints = display(&cfg, &windows, &layout);
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

		let paints = paints
		    .lock()
		    .unwrap();
		
		let paint = paints
		    .iter()
		    .filter(|p: &&Paint| p.win == win && p.left <= x && p.right >= x)
		    .min_by(|p1, p2| (p1.right - p1.left).partial_cmp(&(p2.right - p2.left)).unwrap());

		if let Some(p) = paint {
		    if let Some(cmd) = &p.area.onclick {
			println!("{}", cmd);
		    }
		}
	    }
	    _ => (),
	}
	conn.0.flush();
    }

    Ok(())
}
