#[derive(Debug)]
enum Error {
    XcbConn(xcb::base::ConnError)
}

impl From<xcb::base::ConnError> for Error {
    fn from(e: xcb::base::ConnError) -> Self {
	Self::XcbConn(e)
    }
}

fn main() -> Result<(), Error> {
    let (conn, _) = xcb::Connection::connect(None)?;
    let screen = conn.get_setup().roots().next()
        .expect("Failed to get screen");
    let win = conn.generate_id();

    let (width, height) = (2560, 18);

    xcb::create_window(
	&conn,
	xcb::COPY_FROM_PARENT as u8,
	win,
	screen.root(),
	0,
	14,
	width,
	height,
	0,
	xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
	screen.root_visual(),
	&[
	    (xcb::CW_BACK_PIXEL, screen.white_pixel()),
	]
    );

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
    let cvis = unsafe { cairo::XCBVisualType::from_raw_none(vp) };
    let ccon = unsafe { cairo::XCBConnection::from_raw_none(cp) };
    let cwin = cairo::XCBDrawable(win);

    let surface = cairo::XCBSurface::create(&ccon, &cwin, &cvis, 2560, 20)
	.expect("Failed to create cairo surface");

    xcb::map_window(&conn, win);
    conn.flush();
    
    Ok(())
}
