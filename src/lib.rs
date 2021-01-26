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

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Update {
    pub tag: String,
    #[serde(flatten)]
    pub area: Area,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Draw {
    pub widgets: Vec<Widget>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum Command {
    Update(Update),
    Draw(Draw),
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Widget {
    pub tag: String,
    pub alignment: Alignment,
    #[serde(flatten)]
    pub area: Area,
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct ClickHandler {
    pub button: MouseButton,
    pub output: String,
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Colour {
    pub fn red_fraction(&self) -> f64 {
        let red: f64 = self.red.into();
        red / 255.0
    }

    pub fn green_fraction(&self) -> f64 {
        let green: f64 = self.green.into();
        green / 255.0
    }

    pub fn blue_fraction(&self) -> f64 {
        let blue: f64 = self.blue.into();
        blue / 255.0
    }
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
pub struct Colours {
    #[serde(default)]
    pub fg: Option<Colour>,
    #[serde(default)]
    pub bg: Option<Colour>,
}

impl Default for Colours {
    fn default() -> Self {
        Self { fg: None, bg: None }
    }
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Area {
    pub text: String,
    #[serde(default)]
    pub colours: Colours,
    #[serde(default)]
    pub on_click: Vec<ClickHandler>,
}

pub struct Paint {
    pub left: f64,
    pub right: f64,
    pub win: xcb::Window,
    pub tag: String,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    Left,
    Center,
    Right,
}

impl Alignment {
    pub fn is_center(&self) -> bool {
        match self {
            Self::Center => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    pub fn new(
        x: impl Into<f64>,
        y: impl Into<f64>,
        width: impl Into<f64>,
        height: impl Into<f64>,
    ) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
            width: width.into(),
            height: height.into(),
        }
    }
}

#[derive(Debug)]
pub struct Output {
    pub rect: Rectangle,
    pub win: xcb::Window,
    pub ctx: OutputContext,
}

#[derive(Debug)]
pub struct OutputContext {
    cairo: cairo::Context,
}

#[derive(Debug)]
pub struct Layout {
    pango_layout: pango::Layout,
    pub width: f64,
    pub height: f64,
}

impl Layout {
    pub fn new(ctx: &OutputContext, area: &Area, font: &pango::FontDescription) -> Self {
        let layout =
            pangocairo::create_layout(&ctx.cairo).expect("Failed to create pangocairo layout");

        layout.set_font_description(Some(&font));
        layout.set_text(&area.text);

        let (w, h) = layout.get_pixel_size();
        let area_width: f64 = (w + 10).into();
        let layout_height: f64 = h.into();

        Layout {
            pango_layout: layout,
            width: area_width,
            height: layout_height,
        }
    }

    pub fn display(&self, ctx: &OutputContext) {
        pangocairo::show_layout(&ctx.cairo, &self.pango_layout)
    }
}

impl OutputContext {
    pub fn set_colour(&self, colour: &Colour) {
        self.cairo.set_source_rgb(
            colour.red_fraction(),
            colour.green_fraction(),
            colour.blue_fraction(),
        )
    }

    pub fn fill(&self) {
        self.cairo.fill()
    }

    pub fn rectangle(&self, rect: &Rectangle) {
        self.cairo.rectangle(
            rect.x.into(),
            rect.y.into(),
            rect.width.into(),
            rect.height.into(),
        )
    }

    pub fn move_to(&self, x: f64, y: f64) {
        self.cairo.move_to(x, y)
    }
}

#[derive(Debug)]
pub struct Cursors {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub center: f64,
    pub right: f64,
}

impl Cursors {
    pub fn bump_left(&mut self, by: f64) -> (f64, f64) {
        let old = self.left;
        self.left += by;
        (old, self.left)
    }

    pub fn bump_right(&mut self, by: f64) -> (f64, f64) {
        let old = self.right;
        self.right -= by;
        (self.right, old)
    }

    pub fn bump_center(&mut self, by: f64) -> (f64, f64) {
        let old = self.center;
        self.center += by;
        (old, self.center)
    }

    pub fn make_bounding_rectangle(&mut self, widget: &Widget, layout: &Layout) -> Rectangle {
        let (left, right) = match widget.alignment {
            Alignment::Left => self.bump_left(layout.width),
            Alignment::Right => self.bump_right(layout.width),
            // These are done after all other areas so they can overwrite previously painted areas.
            Alignment::Center => self.bump_center(layout.width),
        };

        Rectangle::new(left, self.top, right - left, self.bottom - self.top)
    }

    pub fn as_rectangle(&self) -> Rectangle {
        Rectangle::new(
            self.left,
            self.top,
            self.right - self.left,
            self.bottom - self.top,
        )
    }
}

pub struct Context<C> {
    config: C,
    outputs: Vec<Output>,
    font: pango::FontDescription,
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
    conn.0
        .get_setup()
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
                (xcb::CW_BACK_PIXEL, screen.black_pixel()),
                (
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_EXPOSURE | xcb::EVENT_MASK_BUTTON_PRESS,
                ),
            ],
        );

        if let [window_type, dock] = &intern_atoms(
            &conn.0,
            &["_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_DOCK"],
        )[..]
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

        let surface = cairo::XCBSurface::create(
            &ccon,
            &cwin,
            &cvis,
            rectangle.width as i32,
            rectangle.height as i32,
        )
        .expect("Failed to create cairo surface");
        let ctx = OutputContext {
            cairo: cairo::Context::new(&surface),
        };

        xcb::map_window(&conn.0, win);

        outputs.push(Output {
            rect: rectangle,
            win,
            ctx,
        })
    }

    outputs.sort_by(|l, r| {
	use std::cmp::Ordering;
	
	if l.rect.y < r.rect.y && l.rect.x < r.rect.y {
	    Ordering::Less
	} else if l.rect.y < r.rect.y || l.rect.x < r.rect.x{
	    Ordering::Less
	} else {
	    Ordering::Less
	}
    });
    
    outputs
}
