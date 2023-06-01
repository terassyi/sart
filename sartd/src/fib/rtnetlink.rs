use std::sync::{Arc, Mutex};

use super::error::Error;



pub(crate) async fn iniit_rtnetlink_handler() -> Result<Arc<Mutex<rtnetlink::Handle>>, Error> {
	let (conn, handler, _rx) = rtnetlink::new_connection().map_err(Error::StdIoErr)?;
	tokio::spawn(conn);
	Ok(Arc::new(Mutex::new(handler)))
}
