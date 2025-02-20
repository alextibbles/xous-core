#![cfg_attr(target_os = "none", no_std)]
pub mod api;
use api::*;
use num_traits::{FromPrimitive, ToPrimitive};
use xous::{msg_scalar_unpack, send_message, Message, CID};
use xous_ipc::Buffer;

pub struct CbTestServer {
    conn: CID,
    tick_cb_sid: Option<xous::SID>,
    req_cb_sid: Option<xous::SID>,
}
static mut REQ_CB: Option<fn(u32)> = None;
impl CbTestServer {
    pub fn new(xns: &xous_names::XousNames) -> Result<Self, xous::Error> {
        let conn = xns
            .request_connection_blocking(api::SERVER_NAME)
            .expect("Can't connect to callback test server");
        Ok(CbTestServer {
            conn,
            tick_cb_sid: None,
            req_cb_sid: None,
        })
    }
    pub fn req(&mut self) -> Result<(), xous::Error> {
        send_message(
            self.conn,
            Message::new_scalar(Opcode::Req.to_usize().unwrap(), 0, 0, 0, 0),
        )
        .map(|_| ())
    }
    pub fn hook_tick_callback(&mut self, id: u32, cid: CID) -> Result<(), xous::Error> {
        if self.tick_cb_sid.is_none() {
            let sid = xous::create_server().unwrap();
            self.tick_cb_sid = Some(sid);
            let sid_tuple = sid.to_u32();
            xous::create_thread_4(
                tick_cb_server,
                sid_tuple.0 as usize,
                sid_tuple.1 as usize,
                sid_tuple.2 as usize,
                sid_tuple.3 as usize,
            )
            .unwrap();
            let hookdata = ScalarHook {
                sid: sid_tuple,
                id,
                cid,
            };
            let buf = Buffer::into_buf(hookdata).or(Err(xous::Error::InternalError))?;
            buf.lend(self.conn, Opcode::RegisterTickListener.to_u32().unwrap())
                .map(|_| ())
        } else {
            Err(xous::Error::MemoryInUse) // can't hook it twice
        }
    }
    pub fn hook_req_callback(&mut self, cb: fn(u32)) -> Result<(), xous::Error> {
        log::info!("hooking req callback");
        if unsafe { REQ_CB }.is_some() {
            return Err(xous::Error::MemoryInUse);
        }
        unsafe { REQ_CB = Some(cb) };
        let sid_tuple: (u32, u32, u32, u32);
        if let Some(sid) = self.req_cb_sid {
            sid_tuple = sid.to_u32();
        } else {
            let sid = xous::create_server().unwrap();
            self.req_cb_sid = Some(sid);
            sid_tuple = sid.to_u32();
            xous::create_thread_4(
                req_cb_server,
                sid_tuple.0 as usize,
                sid_tuple.1 as usize,
                sid_tuple.2 as usize,
                sid_tuple.3 as usize,
            )
            .unwrap();
        }
        xous::send_message(
            self.conn,
            Message::new_scalar(
                Opcode::RegisterReqListener.to_usize().unwrap(),
                sid_tuple.0 as usize,
                sid_tuple.1 as usize,
                sid_tuple.2 as usize,
                sid_tuple.3 as usize,
            ),
        )
        .unwrap();
        Ok(())
    }
    pub fn unhook_req_callback(&mut self) -> Result<(), xous::Error> {
        log::info!("UNhooking req callback");
        unsafe { REQ_CB = None };

        /*
        if let Some(sid) = self.req_cb_sid.take() {
            // tell my handler thread to quit
            log::trace!("connect for unhook");
            let cid = xous::connect(sid).expect("can't connect to CB server for disconnect message");
            log::trace!("sending drop to conn {}", cid);
            send_message(cid,
                Message::new_scalar(ResultCallback::Drop.to_usize().unwrap(), 0, 0, 0, 0)
            ).unwrap();
            log::trace!("disconnecting unhook connection");
            unsafe{
                match xous::disconnect(cid) {
                    Ok(_) => log::trace!("disconnected unhook connection"),
                    Err(e) => log::error!("unhook req got error: {:?}", e),
                };
            }
        }
        log::trace!("nullifying local state");
        self.req_cb_sid = None; */
        if let Some(sid) = self.req_cb_sid {
            let sid_tuple = sid.to_u32();
            xous::send_message(
                self.conn,
                Message::new_scalar(
                    Opcode::UnregisterReqListener.to_usize().unwrap(),
                    sid_tuple.0 as usize,
                    sid_tuple.1 as usize,
                    sid_tuple.2 as usize,
                    sid_tuple.3 as usize,
                ),
            )
            .unwrap();
        }
        Ok(())
    }
}
fn drop_conn(sid: xous::SID, id: usize) {
    let cid = xous::connect(sid).unwrap();
    xous::send_message(cid, Message::new_scalar(id, 0, 0, 0, 0)).unwrap();
    unsafe {
        xous::disconnect(cid).unwrap();
    }
}
impl Drop for CbTestServer {
    fn drop(&mut self) {
        if let Some(sid) = self.req_cb_sid.take() {
            drop_conn(sid, ResultCallback::Drop.to_usize().unwrap());
        }
        if let Some(sid) = self.tick_cb_sid.take() {
            drop_conn(sid, TickCallback::Drop.to_usize().unwrap());
        }
        unsafe {
            xous::disconnect(self.conn).unwrap();
        }
    }
}

fn tick_cb_server(sid0: usize, sid1: usize, sid2: usize, sid3: usize) {
    let sid = xous::SID::from_u32(sid0 as u32, sid1 as u32, sid2 as u32, sid3 as u32);
    loop {
        let msg = xous::receive_message(sid).unwrap();
        match FromPrimitive::from_usize(msg.body.id()) {
            Some(TickCallback::Tick) => msg_scalar_unpack!(msg, cid, id, _, _, {
                // directly pass the scalar message onto the CID with the ID memorized in the original hook
                send_message(cid as u32, Message::new_scalar(id, 0, 0, 0, 0)).unwrap();
            }),
            Some(TickCallback::Drop) => {
                break; // this exits the loop and kills the thread
            }
            None => (),
        }
    }
    xous::destroy_server(sid).unwrap();
}

fn req_cb_server(sid0: usize, sid1: usize, sid2: usize, sid3: usize) {
    let sid = xous::SID::from_u32(sid0 as u32, sid1 as u32, sid2 as u32, sid3 as u32);
    log::info!("req callback server started with SID {:x?}", sid);
    loop {
        let msg = xous::receive_message(sid).unwrap();
        log::trace!("req callback got msg: {:?}", msg);
        match FromPrimitive::from_usize(msg.body.id()) {
            Some(ResultCallback::Result) => msg_scalar_unpack!(msg, result, _, _, _, {
                unsafe {
                    if let Some(cb) = REQ_CB {
                        cb(result as u32)
                    } else {
                        // we got a callback message after we unregistered
                        // not fatal, it just means the server may not have received
                        // our unregistration message in time
                        continue;
                    }
                }
            }),
            Some(ResultCallback::Drop) => {
                break;
            }
            None => {
                log::error!("got unrecognized message in req CB server, ignoring");
            }
        }
    }
    log::info!("req callback server exiting");
    xous::destroy_server(sid).expect("can't destroy my server on exit!");
}
