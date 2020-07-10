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
    Ok(())
}
