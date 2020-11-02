
use actix::{
  Actor, StreamHandler, Addr, Handler
};
use actix_web::{
  web, App, Error, HttpRequest, HttpResponse, HttpServer
};
use actix_web_actors::ws;
use actix_rt;

use actix_derive::{Message, MessageResponse};

use actix::prelude::*;

use rust_embed::RustEmbed;

use serde_json;
use serde_json::json;

use std::sync::{
  Mutex
};

#[derive(RustEmbed)]
#[folder = "src/www"]
struct WWWAssets;

#[derive(Message)]
#[rtype(result = "()")]
pub struct WsMessage(pub String);

/// Define HTTP actor
/// One of these is made for each websocket connection
struct APodWs {
  pub num: usize,
  pub data: web::Data<Mutex<GlobalData>>,
}

impl APodWs {
  pub fn new(data: web::Data<Mutex<GlobalData>>) -> Self {
    let num = data.lock().unwrap().clients.len();
    APodWs {
      num: num,
      data: data
    }
  }
}


impl Actor for APodWs {
    type Context = ws::WebsocketContext<Self>;
}

impl Handler<WsMessage> for APodWs {
    type Result = ();
    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) -> Self::Result {
        // Occurs when a client tells the server something + the server broadcasts.
        // We must forward "msg" to the client's websocket connection.
        let msg: String = msg.0;
        ctx.text(msg);
    }
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for APodWs {
    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.data.lock().unwrap().clients.push(
          addr.recipient()
        );
    }

    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => handle_ws_msg(self, ctx, text), //ctx.text(text),
            Ok(ws::Message::Binary(bin)) => (),
            _ => (),
        }
    }
}

struct GlobalData {
  pub clients: Vec<Recipient<WsMessage>>
}

impl Default for GlobalData {
  fn default() -> Self {
    println!("GlobalData::default run!");
    GlobalData {
      clients: vec![]
    }
  }
}

fn handle_ws_msg(ws: &mut APodWs, ctx: &mut ws::WebsocketContext<APodWs>, text: String) {
  // Parse JSON
  let json: serde_json::Result<serde_json::Value> = serde_json::from_str(&text[..]);
  let json = match json {
    Err(e) => { return; },
    Ok(j) => j,
  };

  // Anytime someone sends the server data we forward it to everyone else,
  // including the sender.
  for client in &ws.data.lock().unwrap().clients {
    if let Err(e) = client.try_send(WsMessage(text.clone())) {
      println!("Error sending msg to client: {}", e);
    }
  }

  if json["event"] == json!("leader-joined") {
    // Lookup our LAN IP and send it to the leader
    ctx.text(format!(r#"{{ "event":"lan-ip", "ip": "{}" }}"#, get_lan_ip()));
  }

}

// This fn upgrades /ws/ http requests to a websocket connection
// which may stream events to/from the GUI
async fn ws_handler(req: HttpRequest, stream: web::Payload, data: web::Data<Mutex<GlobalData>>) -> Result<HttpResponse, Error> {
    let resp = ws::start(APodWs::new(data), &req, stream);
    //println!("{:?}", resp);
    resp
}

// This fn grabs assets and returns them
async fn index(req: HttpRequest, _stream: web::Payload) -> HttpResponse {
  
  // We perform some common routing tactics here
  let mut r_path = req.path();
  if r_path == "/" {
    r_path = "index.html";
  }
  if r_path.starts_with("/") {
    r_path = &r_path[1..];
  }

  // Do some security checks (only localhost should talk to "leader.html")
  if r_path == "leader.html" {
    if let Some(addr) = req.peer_addr() {
      if ! addr.ip().is_loopback() {
        // Security error, don't let anyone become the leader!
        return HttpResponse::NotFound()
          .content_type("text/html")
          .body(&include_bytes!("www/404.html")[..]);
      }
    }
  }

  // Finally pull from fs/memory 
  match WWWAssets::get(r_path) {
    Some(data) => {
      // Figure out MIME from file extension
      let guess = mime_guess::from_path(r_path);
      let mime_s = guess.first_raw().unwrap_or("application/octet-stream");
      let owned_data: Vec<u8> = (&data[..]).iter().cloned().collect();
      HttpResponse::Ok()
            .content_type(mime_s)
            .body(owned_data)
    }
    None => {
      HttpResponse::NotFound()
        .content_type("text/html")
        .body(&include_bytes!("www/404.html")[..])
    }
  }
}

pub fn main() -> Result<(), Box<dyn std::error::Error>>  {

  let local_ip = get_lan_ip();
  println!("local_ip={}", local_ip); // TODO store globally so leader ws can ask for it

  let sys = actix_rt::System::new(crate::APP_NAME);
  
  let address = format!("0.0.0.0:{}", crate::HTTP_PORT);

  HttpServer::new(||
      App::new()
        .app_data( web::Data::new( Mutex::new( GlobalData::default() ) ) )
        .route("/ws", web::get().to(ws_handler))
        .route("/", web::get().to(index))
        .default_service(
          web::route().to(index)
        )

    )
    .workers(1)
    .backlog(16)
    .bind(&address)?
    .run();

  let x = sys.run()?;
  println!("x={:?}", x); // paranoia about smart compiler optimizations

  Ok(())
}

fn get_lan_ip() -> String {
  use std::net::UdpSocket;
  let socket = match UdpSocket::bind("0.0.0.0:0") {
      Ok(s) => s,
      Err(_) => return "127.0.0.1".to_string(),
  };

  match socket.connect("8.8.8.8:80") {
      Ok(()) => (),
      Err(_) => return "127.0.0.1".to_string(),
  };

  match socket.local_addr() {
      Ok(addr) => addr.ip().to_string(),
      Err(_) => "127.0.0.1".to_string(),
  }
}


