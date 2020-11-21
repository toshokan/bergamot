use bergamot::{create_output_windows, get_connection, get_rectangles, get_screen};
use bergamot::{error::Error, Align, Area, Colour, Output, Paint};
use std::sync::{mpsc::channel, Arc, Mutex};

struct Config {
    height: u32,
    font_str: String,
    bg: Colour,
}

#[derive(Debug)]
struct Layout {
    pango_layout: pango::Layout,
    width: f64,
    height: f64,
}

fn create_layout(output: &Output, area: &Area, font: &pango::FontDescription) -> Layout {
    let layout = pangocairo::create_layout(&output.ctx)
        .expect("Failed to create pangocairo layout");
    layout.set_font_description(Some(&font));
    layout.set_text(&area.text);

    let (w, h) = layout.get_pixel_size();
    let area_width: f64 = (w + 10).into();
    let layout_height: f64 = h.into();
    
    Layout {
	pango_layout: layout,
	width: area_width,
	height: layout_height
    }
}

fn display(cfg: &Config, outputs: &[Output], areas: &[Area]) -> Vec<Paint> {
    let mut area_paints = vec![];
    let font = pango::FontDescription::from_string(&cfg.font_str);

    let (centered_areas, other_areas): (Vec<&Area>, Vec<&Area>) = areas.iter().partition(|a| a.align.is_center());

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

	let centered_areas: Vec<(&&Area, Layout)> = centered_areas.iter().map(|a| (a, create_layout(output, a, &font))).collect();
	let other_areas: Vec<(&&Area, Layout)> = other_areas.iter().map(|a| (a, create_layout(output, a, &font))).collect();

	let center_width: f64 = centered_areas.iter().map(|(_, l)| l.width).sum();
	let mut center_finger = (output.rect.width / 2) as f64 - center_width;

        for (area, layout) in other_areas.iter().chain(centered_areas.iter()) {

            let bg = area.bg.as_ref().unwrap_or(&Colour {
                red: 0_f64,
                blue: 0_f64,
                green: 0_f64,
            });

            output.ctx.set_source_rgb(bg.red, bg.green, bg.blue);
            let (left, right) = match area.align {
                Align::Left => {
                    let (left, right) = (left_finger, left_finger + layout.width);
                    left_finger += layout.width;
                    (left, right)
                },
                Align::Right => {
                    let (left, right) = (right_finger - layout.width, right_finger);
                    right_finger -= layout.width;
                    (left, right)
                },
		// These are done after all other areas so they can overwrite previously painted areas.
		Align::Center => {
		    let (left, right) = (center_finger, center_finger + layout.width);
		    center_finger += layout.width;
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
                .move_to(left + 5_f64, height / 2_f64 - layout.height / 2_f64);
            pangocairo::show_layout(&output.ctx, &layout.pango_layout);

            area_paints.push(Paint { left, right, win: output.win, area: (**area).clone() })
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
    let layout = Arc::new(Mutex::new(Vec::new()));
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
			let value: Result<Vec<Area>, _> = serde_json::from_str(&buf);
			if let Ok(new_layout) = value {
			    let mut layout = layout.lock().unwrap();
			    let _ = std::mem::replace(&mut *layout, new_layout);
			    buf.clear();

			    tx.send(()).unwrap();
			} else {
			    eprintln!("{:?}", value);
			}
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
