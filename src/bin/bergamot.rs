#[derive(Debug)]
enum Error {
    XcbConn(xcb::base::ConnError),
}

impl From<xcb::base::ConnError> for Error {
    fn from(e: xcb::base::ConnError) -> Self {
        Self::XcbConn(e)
    }
}

fn intern_atoms(conn: &'_ xcb::Connection, names: &[&str]) -> Vec<xcb::InternAtomReply> {
    names
        .iter()
        .map(|n| xcb::intern_atom(&conn, false, n))
        .map(|c| c.get_reply().expect("Bad reply"))
        .collect()
}

#[derive(Debug)]
struct Rectangle {
    start: u32,
    stop: u32,
}

#[derive(Debug)]
struct Area {
    align: Align,
    text: String,
    tag: String,
    fg: Colour,
    bg: Option<Colour>,
    onclick: Option<String>,
}

struct Paint {
    left: f64,
    right: f64,
    area: Area,
}

#[derive(Debug)]
enum Align {
    Left,
    Right,
}

#[derive(Debug)]
struct Colour {
    red: f64,
    green: f64,
    blue: f64,
}

fn main() -> Result<(), Error> {
    let (conn, _) = xcb::Connection::connect(None)?;
    let screen = conn
        .get_setup()
        .roots()
        .next()
        .expect("Failed to get screen");
    let win = conn.generate_id();

    let (width, height) = (screen.width_in_pixels(), 18);

    xcb::create_window(
        &conn,
        xcb::COPY_FROM_PARENT as u8,
        win,
        screen.root(),
        0,
        20,
        width,
        height,
        0,
        xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
        screen.root_visual(),
        &[
            (xcb::CW_BACK_PIXEL, screen.white_pixel()),
            (xcb::CW_EVENT_MASK, xcb::EVENT_MASK_EXPOSURE),
        ],
    );

    if let [window_type, dock] =
        &intern_atoms(&conn, &["_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_DOCK"])[..]
    {
        xcb::change_property(
            &conn,
            xcb::PROP_MODE_REPLACE as u8,
            win,
            window_type.atom(),
            xcb::ATOM_ATOM,
            32,
            &[dock.atom()],
        );
    }

    let visp = screen
        .allowed_depths()
        .next()
        .expect("No allowed depths")
        .visuals()
        .next()
        .expect("No visuals")
        .base;

    let vp = &visp as *const xcb::ffi::xproto::xcb_visualtype_t as *mut cairo_sys::xcb_visualtype_t;
    let cp = conn.get_raw_conn() as *mut cairo_sys::xcb_connection_t;
    let cvis = unsafe { cairo::XCBVisualType::from_raw_borrow(vp) };
    let ccon = unsafe { cairo::XCBConnection::from_raw_borrow(cp) };
    let cwin = cairo::XCBDrawable(win);

    let surface = cairo::XCBSurface::create(&ccon, &cwin, &cvis, width.into(), height.into())
        .expect("Failed to create cairo surface");
    let ctx = cairo::Context::new(&surface);

    xcb::map_window(&conn, win);
    conn.flush();

    let mut area_paints = vec![];

    while let Some(event) = conn.wait_for_event() {
        match event.response_type() & !0x80 {
            xcb::EXPOSE => {
                let font = pango::FontDescription::from_string("monospace 12");

                let (width, height) = (width.into(), height.into());
                ctx.set_source_rgb(0.1, 0.1, 0.1);
                ctx.rectangle(0.0, 0.0, width, height);
                ctx.fill();

                let mut areas = vec![
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
                        onclick: None,
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
                        onclick: None,
                    },
                ];

                let mut left_finger = 0_f64;
                let mut right_finger = width;
                for area in areas.drain(..) {
                    let layout = pangocairo::create_layout(&ctx)
                        .expect("Failed to create pangocairo layout");
		    layout.set_font_description(Some(&font));
		    layout.set_text(&area.text);

		    let (w, h) = layout.get_pixel_size();
		    let area_width: f64 = (w + 10).into();
		    let layout_height: f64 = h.into();

		    let bg = area.bg.as_ref().unwrap_or(&Colour {
			red: 0_f64,
			blue: 0_f64,
			green: 0_f64
		    });

                    ctx.set_source_rgb(bg.red, bg.green, bg.blue);
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
                    ctx.rectangle(left, 0_f64, right - left, height);
		    ctx.fill();

		    ctx.set_source_rgb(area.fg.red, area.fg.green, area.fg.blue);
		    ctx.move_to(left + 5_f64, height/2_f64 - layout_height/2_f64);
		    pangocairo::show_layout(&ctx, &layout);

                    area_paints.push(Paint { left, right, area })
                }
            }
            _ => (),
        }
        conn.flush();
    }

    Ok(())
}
