
use actix::{Actor, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use actix_rt;

use rust_embed::RustEmbed;


#[derive(RustEmbed)]
#[folder = "src/www"]
struct WWWAssets;

/// Define HTTP actor
struct MyWs;

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => ctx.text(text),
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            _ => (),
        }
    }
}

// This fn upgrades /ws/ http requests to a websocket connection
// which may stream events to/from the GUI
async fn ws_handler(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    println!("ws_handler");
    let resp = ws::start(MyWs {}, &req, stream);
    println!("{:?}", resp);
    resp
}

// This fn grabs assets and returns them
async fn index(req: HttpRequest, _stream: web::Payload) -> HttpResponse {
  use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
        .route("/ws", web::get().to(ws_handler))
        .route("/", web::get().to(index))
        .default_service(
          web::route().to(index)
        )

    )
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


