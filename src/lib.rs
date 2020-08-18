pub mod error {
    #[derive(Debug)]
    pub enum Error {
	XcbConn(xcb::base::ConnError),
	XcbGeneric(xcb::base::GenericError),
    }

    impl From<xcb::base::ConnError> for Error {
	fn from(e: xcb::base::ConnError) -> Self {
            Self::XcbConn(e)
	}
    }

    impl From<xcb::base::GenericError> for Error {
	fn from(e: xcb::base::GenericError) -> Self {
            Self::XcbGeneric(e)
	}
    }
}

#[derive(Debug)]
pub struct Area {
    pub align: Align,
    pub text: String,
    pub tag: String,
    pub fg: Colour,
    pub bg: Option<Colour>,
    pub onclick: Option<String>,
}

pub struct Paint {
    pub left: f64,
    pub right: f64,
    pub area: Area,
}

#[derive(Debug)]
pub enum Align {
    Left,
    Right,
}

#[derive(Debug)]
pub struct Colour {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
}

#[derive(Debug)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct Output {
    pub rect: Rectangle,
    pub ctx: cairo::Context,
}


pub fn get_connection() -> Result<xcb::Connection, error::Error> {
    let (conn, _) = xcb::Connection::connect(None)?;
    Ok(conn)
}

pub fn get_screen(conn: &'_ xcb::Connection) -> xcb::Screen<'_> {
    conn
        .get_setup()
        .roots()
        .next()
        .expect("Failed to get screen")
}

pub fn get_rectangles(conn: &xcb::Connection, screen: &xcb::Screen<'_>) -> Result<Vec<Rectangle>, error::Error> {
    let present = xcb::xproto::query_extension(conn, "RANDR")
        .get_reply()?
        .present();

    if !present {
        unimplemented!("RANDR must be present");
    }

    let resources = xcb::randr::get_screen_resources_current(&conn, screen.root()).get_reply()?;

    let outputs = resources.outputs();

    let mut crtcs = Vec::new();

    for output in outputs {
        let info = xcb::randr::get_output_info(&conn, *output, xcb::CURRENT_TIME).get_reply()?;

        if info.crtc() == xcb::base::NONE
            || Into::<u32>::into(info.connection()) == xcb::randr::CONNECTION_DISCONNECTED
        {
            continue;
        } else {
            let cookie = xcb::randr::get_crtc_info(&conn, info.crtc(), xcb::CURRENT_TIME);
            crtcs.push(cookie);
        }
    }

    let mut rectangles = Vec::new();

    for crtc in crtcs {
        let info = crtc.get_reply()?;
        let rect = Rectangle {
            x: info.x().into(),
            y: info.y().into(),
            width: info.width().into(),
            height: info.height().into(),
        };
        rectangles.push(rect);
    }

    Ok(rectangles)
}

fn intern_atoms(conn: &'_ xcb::Connection, names: &[&str]) -> Vec<xcb::InternAtomReply> {
    names
        .iter()
        .map(|n| xcb::intern_atom(&conn, false, n))
        .map(|c| c.get_reply().expect("Bad reply"))
        .collect()
}


pub fn create_output_windows(conn: &xcb::Connection, screen: &xcb::Screen<'_>, bar_height: i32, rectangles: Vec<Rectangle>) -> Vec<Output> {
    use std::convert::TryInto;
    
    let mut outputs = Vec::new();

    for rectangle in rectangles {
        let win = conn.generate_id();

        let y: i16 = rectangle.y.try_into().unwrap();

        xcb::create_window(
            &conn,
            xcb::COPY_FROM_PARENT as u8,
            win,
            screen.root(),
            rectangle.x.try_into().unwrap(),
            30 + y,
            rectangle.width.try_into().unwrap(),
            bar_height.try_into().unwrap(),
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

        let vp = &visp as *const _ as *mut _;
        let cp = conn.get_raw_conn() as *mut _;

        let cvis = unsafe { cairo::XCBVisualType::from_raw_borrow(vp) };
        let ccon = unsafe { cairo::XCBConnection::from_raw_borrow(cp) };
        let cwin = cairo::XCBDrawable(win);

        let surface =
            cairo::XCBSurface::create(&ccon, &cwin, &cvis, rectangle.width, rectangle.height)
            .expect("Failed to create cairo surface");
        let ctx = cairo::Context::new(&surface);

        xcb::map_window(&conn, win);

        outputs.push(Output {
            rect: rectangle,
            ctx,
        })
    }

    outputs
}
