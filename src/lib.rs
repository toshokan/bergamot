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

#[derive(serde::Deserialize)]
#[derive(Debug, Clone)]
pub struct Area {
    pub align: Align,
    pub text: String,
    pub tag: String,
    pub fg: Colour,
    pub bg: Option<Colour>,
    pub on_click: Option<String>,
    pub on_middle_click: Option<String>,
    pub on_right_click: Option<String>,
}

pub struct Paint {
    pub left: f64,
    pub right: f64,
    pub win: xcb::Window,
    pub area: Area,
}

#[derive(serde::Deserialize)]
#[derive(Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    pub fn is_center(&self) -> bool {
	match self {
	    Align::Center => true,
	    _ => false
	}
    }
}

#[derive(serde::Deserialize)]
#[derive(Debug, Clone)]
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
    pub win: xcb::Window,
    pub ctx: cairo::Context,
}

unsafe impl Send for Output {}

pub struct XcbConnection(pub xcb::Connection);
unsafe impl Send for XcbConnection {}
unsafe impl Sync for XcbConnection {}

impl XcbConnection {
    pub fn flush(&self) {
	self.0.flush();
    }
}


pub fn get_connection() -> Result<XcbConnection, error::Error> {
    let (conn, _) = xcb::Connection::connect(None)?;
    Ok(XcbConnection(conn))
}

pub fn get_screen(conn: &'_ XcbConnection) -> xcb::Screen<'_> {
    conn.0.get_setup()
        .roots()
        .next()
        .expect("Failed to get screen")
}

pub fn get_rectangles(
    conn: &XcbConnection,
    screen: &xcb::Screen<'_>,
) -> Result<Vec<Rectangle>, error::Error> {
    let present = xcb::xproto::query_extension(&conn.0, "RANDR")
        .get_reply()?
        .present();
    
    if !present {
        unimplemented!("RANDR must be present");
    }

    let resources = xcb::randr::get_screen_resources_current(&conn.0, screen.root()).get_reply()?;

    let outputs = resources.outputs();

    let mut crtcs = Vec::new();

    for output in outputs {
        let info = xcb::randr::get_output_info(&conn.0, *output, xcb::CURRENT_TIME).get_reply()?;

        if info.crtc() == xcb::base::NONE
            || Into::<u32>::into(info.connection()) == xcb::randr::CONNECTION_DISCONNECTED
        {
            continue;
        } else {
            let cookie = xcb::randr::get_crtc_info(&conn.0, info.crtc(), xcb::CURRENT_TIME);
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

pub fn create_output_windows(
    conn: &XcbConnection,
    screen: &xcb::Screen<'_>,
    bar_height: i32,
    rectangles: Vec<Rectangle>,
) -> Vec<Output> {
    let mut outputs = Vec::new();

    for rectangle in rectangles {
        let win = conn.0.generate_id();

        xcb::create_window(
            &conn.0,
            xcb::COPY_FROM_PARENT as u8,
            win,
            screen.root(),
            rectangle.x as i16,
            (rectangle.y as i16) + 30,
            rectangle.width as u16,
            bar_height as u16,
            0,
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(),
            &[
                (xcb::CW_BACK_PIXEL, screen.white_pixel()),
                (xcb::CW_EVENT_MASK, xcb::EVENT_MASK_EXPOSURE | xcb::EVENT_MASK_BUTTON_PRESS),
            ],
        );

        if let [window_type, dock] =
            &intern_atoms(&conn.0, &["_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_DOCK"])[..]
        {
            xcb::change_property(
                &conn.0,
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
        let cp = conn.0.get_raw_conn() as *mut _;

        let cvis = unsafe { cairo::XCBVisualType::from_raw_borrow(vp) };
        let ccon = unsafe { cairo::XCBConnection::from_raw_borrow(cp) };
        let cwin = cairo::XCBDrawable(win);

        let surface =
            cairo::XCBSurface::create(&ccon, &cwin, &cvis, rectangle.width, rectangle.height)
                .expect("Failed to create cairo surface");
        let ctx = cairo::Context::new(&surface);

        xcb::map_window(&conn.0, win);

        outputs.push(Output {
            rect: rectangle,
	    win,
            ctx,
        })
    }

    outputs
}
