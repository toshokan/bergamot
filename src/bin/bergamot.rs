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

    

    xcb::map_window(&conn, win);
    conn.flush();
    
    Ok(())
}
