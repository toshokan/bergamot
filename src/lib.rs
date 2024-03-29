use xcb::x::{Window, Screen, InternAtomReply};
use xcb::Xid;

pub mod error {
    #[derive(Debug)]
    pub enum Error {
	Xcb(xcb::Error)
    }

    impl From<xcb::Error> for Error {
        fn from(e: xcb::Error) -> Self {
            Self::Xcb(e)
        }
    }

    impl From<xcb::ConnError> for Error {
	fn from(e: xcb::ConnError) -> Self {
	    Self::Xcb(xcb::Error::Connection(e))
	}
    }

}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all = "lowercase")]
pub enum Constraint {
    Monitor(MonitorConstraint),
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
#[serde(transparent)]
pub struct MonitorConstraint(usize);

impl MonitorConstraint {
    pub fn number(&self) -> usize {
        self.0
    }
}

#[derive(serde::Deserialize, Debug, Default, Clone)]
#[serde(transparent)]
pub struct Constraints(Vec<Constraint>);

impl Constraints {
    pub fn monitor(&self) -> impl Iterator<Item = MonitorConstraint> + '_ {
        self.0.iter().filter_map(|c| match c {
            Constraint::Monitor(m) => Some(*m),
        })
    }
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Update {
    pub tag: String,
    pub content: Vec<Area>,
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
#[serde(deny_unknown_fields)]
pub struct Widget {
    #[serde(default)]
    pub tag: String,
    pub alignment: Alignment,
    #[serde(default)]
    pub content: Vec<Area>,
    #[serde(default)]
    pub constraints: Constraints,
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct ClickHandler {
    pub button: MouseButton,
    pub output: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Colour {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

pub struct BadHexFormat(String);

impl std::fmt::Debug for BadHexFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bad hex format {}", self.0)
    }
}

impl std::str::FromStr for Colour {
    type Err = BadHexFormat;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        fn byte(s: &str) -> Result<u8, BadHexFormat> {
            u8::from_str_radix(s, 16).map_err(|_| BadHexFormat(s.to_string()))
        }

        if value.len() == 7 && &value[0..1] == "#" {
            Ok(Colour {
                red: byte(&value[1..3])?,
                green: byte(&value[3..5])?,
                blue: byte(&value[5..7])?,
            })
        } else {
            Err(BadHexFormat(value.to_string()))
        }
    }
}

impl<'de> serde::Deserialize<'de> for Colour {
    fn deserialize<D>(deserializer: D) -> Result<Colour, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Error, MapAccess};
        use std::fmt;

        struct RgbOrHex;
        #[derive(serde::Deserialize)]
        pub struct RawColour {
            pub red: u8,
            pub green: u8,
            pub blue: u8,
        }

        impl<'de> de::Visitor<'de> for RgbOrHex {
            type Value = Colour;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.write_str("RGB map or hex colour code")
            }

            fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
                use std::str::FromStr;

                Colour::from_str(value).map_err(|e| Error::custom(format!("{:?}", e)))
            }

            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                use de::value::MapAccessDeserializer;
                use serde::Deserialize;

                let rc = RawColour::deserialize(MapAccessDeserializer::new(map))?;
                Ok(Colour {
                    red: rc.red,
                    green: rc.green,
                    blue: rc.blue,
                })
            }
        }

        deserializer.deserialize_any(RgbOrHex)
    }
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

#[derive(serde::Deserialize, Default, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Area {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub colours: Colours,
    #[serde(default)]
    pub on_click: Vec<ClickHandler>,
}

#[derive(Debug)]
pub struct Paint {
    pub left: f64,
    pub right: f64,
    pub win: Window,
    pub area: Area,
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

    pub fn is_right(&self) -> bool {
        match self {
            Self::Right => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
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
    pub win: Window,
    pub ctx: OutputContext,
    pub font: FontDescription,
    pub cfg: Config
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

        let (w, h) = layout.pixel_size();
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
        self.cairo.fill().expect("Failed to fill");
    }

    pub fn rectangle(&self, rect: &Rectangle) {
        self.cairo.rectangle(
            rect.x.into(),
            rect.y.into(),
            rect.width.into(),
            rect.height.into(),
        )
    }

    pub fn status(&self) {
	let s = self.cairo.target();
	s.flush();
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
        self.right += by;
        (old, self.right)
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

#[derive(Debug, Clone)]
pub struct Config {
    pub height: u32,
    pub font_str: String,
    pub default_bg: Colour,
    pub default_fg: Colour,
}

unsafe impl Send for Output {}

pub struct XcbConnection(pub xcb::Connection);
unsafe impl Send for XcbConnection {}
unsafe impl Sync for XcbConnection {}

#[derive(Debug, Clone)]
pub struct FontDescription(pub pango::FontDescription);

impl FontDescription {
    pub fn new(description: impl AsRef<str>) -> Self {
        let fd = pango::FontDescription::from_string(description.as_ref());
        Self(fd)
    }
}
unsafe impl Send for FontDescription {}

impl XcbConnection {
    pub fn flush(&self) {
        self.0.flush().expect("Failed to flush connection");
    }
}

pub fn get_connection() -> Result<XcbConnection, error::Error> {
    let (conn, _) = xcb::Connection::connect_with_extensions(
	None,
	&[xcb::Extension::RandR],
	&[]
    )?;
    Ok(XcbConnection(conn))
}

pub fn get_screen(conn: &'_ XcbConnection) -> &'_ Screen {
    conn.0
        .get_setup()
        .roots()
        .next()
        .expect("Failed to get screen")
}

pub fn get_rectangles(
    conn: &XcbConnection,
    screen: &Screen,
) -> Result<Vec<Rectangle>, error::Error> {

    let resources = conn.0.wait_for_reply(conn.0.send_request(&xcb::randr::GetScreenResourcesCurrent {
	window: screen.root()
    }))?;


    let outputs = resources.outputs();

    let mut crtcs = Vec::new();

    for output in outputs {
	let info = conn.0.wait_for_reply(conn.0.send_request(&xcb::randr::GetOutputInfo {
	    output: *output,
	    config_timestamp: xcb::x::CURRENT_TIME
	}))?;

        if info.crtc().is_none() || info.connection() == xcb::randr::Connection::Disconnected
        {
            continue;
        } else {
	    let cookie = conn.0.send_request(&xcb::randr::GetCrtcInfo {
		crtc: info.crtc(),
		config_timestamp: xcb::x::CURRENT_TIME,
	    });
            crtcs.push(cookie);
        }
    }

    let mut rectangles = Vec::new();

    for crtc in crtcs {
        let info = conn.0.wait_for_reply(crtc)?;
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

fn intern_atoms(conn: &'_ xcb::Connection, names: &[&str]) -> Vec<InternAtomReply> {
    names
        .iter()
        .map(|n| conn.send_request(&xcb::x::InternAtom {
	    only_if_exists: false,
	    name: &n.as_bytes()
	}))
        .map(|c| conn.wait_for_reply(c).expect("Bad reply"))
        .collect()
}

pub fn create_output_windows(
    conn: &XcbConnection,
    screen: &Screen,
    configs: &Vec<Config>,
    mut rectangles: Vec<Rectangle>,
) -> Vec<Output> {
    let mut outputs = Vec::new();

    rectangles.sort_by(|l, r| {
        use std::cmp::Ordering;

        if l.y < r.y && l.x < r.y {
            Ordering::Less
        } else if l.y < r.y || l.x < r.x {
            Ordering::Less
        } else {
            Ordering::Less
        }
    });

    for (rectangle, config) in rectangles.iter().zip(configs) {
        let win: Window = conn.0.generate_id();

	conn.0.send_request(&xcb::x::CreateWindow {
	    depth: xcb::x::COPY_FROM_PARENT as u8,
	    wid: win,
	    parent: screen.root(),
	    x: rectangle.x as i16,
	    y: rectangle.y as i16,
	    width: rectangle.width as u16,
	    height: config.height as u16,
	    border_width: 0,
	    class: xcb::x::WindowClass::InputOutput,
	    visual: screen.root_visual(),
	    value_list: &[
                xcb::x::Cw::BackPixel(screen.black_pixel()),
		xcb::x::Cw::EventMask(xcb::x::EventMask::EXPOSURE | xcb::x::EventMask::BUTTON_PRESS)
            ],
	});

        if let [window_type, dock, state, below, strut, strut_partial] = &intern_atoms(
            &conn.0,
            &[
                "_NET_WM_WINDOW_TYPE",
                "_NET_WM_WINDOW_TYPE_DOCK",
                "_NET_WM_STATE",
                "_NET_WM_STATE_BELOW",
		"_NET_WM_STRUT",
                "_NET_WM_STRUT_PARTIAL",
            ],
        )[..]
        {
	    conn.0.send_request(&xcb::x::ChangeProperty {
		mode: xcb::x::PropMode::Replace,
		window: win,
		property: window_type.atom(),
		r#type: xcb::x::ATOM_ATOM,
		data: &[dock.atom()]
	    });
	    conn.0.send_request(&xcb::x::ChangeProperty {
		mode: xcb::x::PropMode::Replace,
		window: win,
		property: state.atom(),
		r#type: xcb::x::ATOM_ATOM,
		data: &[below.atom()]
	    });
	    conn.0.send_request(&xcb::x::ChangeProperty {
		mode: xcb::x::PropMode::Replace,
		window: win,
		property: strut.atom(),
		r#type: xcb::x::ATOM_CARDINAL,
		data: &[
		    0, //left
                    0, //right
		    config.height, //top
		    0, //bottom
		]
	    });
	    conn.0.send_request(&xcb::x::ChangeProperty {
		mode: xcb::x::PropMode::Replace,
		window: win,
		property: strut_partial.atom(),
		r#type: xcb::x::ATOM_CARDINAL,
		data: &[
		    0, //left
                    0, //right
		    config.height, //top
		    0, //bottom
		    0, //left_start_y
		    0, //left_end_y
		    0, // right_start_y
		    0, // right_end_y
		    rectangle.x as u32, // top_start_x
		    (rectangle.x + rectangle.width) as u32, // top_end_x
		    0, // bottom_start_x
		    0, // bottom_end_x
		]
	    });
	    conn.0.send_request(&xcb::x::ChangeProperty {
		mode: xcb::x::PropMode::Replace,
		window: win,
		property: xcb::x::ATOM_WM_CLASS,
		r#type: xcb::x::ATOM_STRING,
		data: "bergamot\0bergamot".as_bytes(),
	    });
        }

        let visp = screen
            .allowed_depths()
            .next()
            .expect("No allowed depths")
            .visuals()
	    .iter()
	    .next()
            .expect("No visuals");

	let cvis = unsafe {
	    cairo::XCBVisualType::from_raw_none(
		visp as *const _ as *mut xcb::x::Visualtype as *mut _)
	};
	let ccon = unsafe {
	    cairo::XCBConnection::from_raw_none(conn.0.get_raw_conn() as *mut _)
	};
	
        let cwin = cairo::XCBDrawable(win.resource_id());

        let surface = cairo::XCBSurface::create(
            &ccon,
            &cwin,
            &cvis,
            rectangle.width as i32,
            rectangle.height as i32,
        ).expect("Failed to create cairo surface");

	let cctx = cairo::Context::new(&surface)
	    .expect("Failed to create cairo context");
	
        let ctx = OutputContext {
            cairo: cctx
        };

	conn.0.send_request(&xcb::x::MapWindow {
	    window: win
	});

	let font = FontDescription::new(&config.font_str);

        outputs.push(Output {
            rect: rectangle.clone(),
            win,
            ctx,
	    font,
	    cfg: config.clone()
        })
    }

    outputs
}
