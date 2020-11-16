
use actix::{
  Actor, StreamHandler, Handler
};
use actix_web::{
  web, App, Error, HttpRequest, HttpResponse, HttpServer
};
use actix_web_actors::ws;
use actix_rt;

use actix_derive::{Message};

use actix::prelude::*;

use rust_embed::RustEmbed;

use serde_json;
use serde_json::json;

use openssl::ssl::*;
use openssl::pkey::{ PKey };
use openssl::rsa::{ Rsa };

use std::env;
use std::sync::{
  Mutex
};
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::io::prelude::*;

use crate::gui;

#[derive(RustEmbed)]
#[folder = "src/www"]
struct WWWAssets;

#[derive(Message)]
#[rtype(result = "()")]
pub enum WsMessage {
  S(String),
  B(Vec<u8>)
}

/// Define HTTP actor
/// One of these is made for each websocket connection
struct APodWs {
  // Index in GlobalData.clients
  pub num: usize,
  // Set on connection, is true if localhost
  pub is_leader: bool,
  // A pointer to all the other clients via GlobalData
  pub data: web::Data<Mutex<GlobalData>>,
}

impl APodWs {
  pub fn new(data: web::Data<Mutex<GlobalData>>) -> Self {
    let num = data.lock().unwrap().clients.len();
    APodWs {
      num: num,
      is_leader: false,
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
        match msg {
          WsMessage::S(msg) => {
            ctx.text(msg);
          }
          WsMessage::B(bin) => {
            ctx.binary(bin);
          }
        }
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
        //println!("handle msg={:?}", &msg);
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => handle_ws_msg(self, ctx, text), //ctx.text(text),
            Ok(ws::Message::Binary(bin)) => handle_ws_bin(self, ctx, &bin[..]),
            _ => (),
        }
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
      ctx.stop();
    }
}

struct GlobalData {
  pub clients: Vec<Recipient<WsMessage>>,
  pub save_dir: PathBuf,
}

impl Default for GlobalData {
  fn default() -> Self {
    println!("GlobalData::default run!");
    //let mut save_dir = env::current_dir().expect("Could not get current_dir");
    let mut save_dir: PathBuf;
    if let Some(arg) = env::args().skip(1).next() {
      save_dir = PathBuf::from(arg);
    } else {
      // Open GUI to get directory, do not let app continue until user has given us data.
      loop {
        match gui::fork_ask_for_dir() {
          Some(directory) => {
            save_dir = directory;
            break;
          }
          None => {
            println!("You must select a save directory.");
          }
        }
      }
    };
    println!("initial save_dir={:?}", &save_dir.as_path().to_string_lossy());
    GlobalData {
      clients: vec![],
      save_dir: save_dir
    }
  }
}

fn handle_ws_bin(ws: &mut APodWs, _ctx: &mut ws::WebsocketContext<APodWs>, bin: &[u8]) {
  // No longer used, instead we POST 500ms of data at a time to /save
}

fn handle_ws_msg(ws: &mut APodWs, ctx: &mut ws::WebsocketContext<APodWs>, text: String) {
  println!("ws text={}", &text[..]);
  // Parse JSON
  let json: serde_json::Result<serde_json::Value> = serde_json::from_str(&text[..]);
  let json = match json {
    Err(_e) => { return; },
    Ok(j) => j,
  };

  // Anytime someone sends the server data we forward it to everyone else,
  // excluding the sender

  {
    let clients = &mut ws.data.lock().unwrap().clients;
    let mut idx_to_rm: Option<usize> = None;
    for i in 0..clients.len() {
      // if i == ws.num {
      //   continue;
      // }
      if let Err(e) = clients[i].try_send(WsMessage::S(text.clone())) {
        println!("Error sending text to client: {}", e);
        idx_to_rm = Some(i);
      }
    }
    if let Some(idx_to_rm) = idx_to_rm {
      clients.remove(idx_to_rm);
    }
  }

  if ws.is_leader {
    // Process leader-specific commands
    if json["event"] == json!("leader-joined") {
      // Lookup our LAN IP and send it to the leader
      ctx.text(format!(r#"{{ "event":"lan-ip", "ip": "{}" }}"#, get_lan_ip()));
      // Tell leader about save dir
      {
        let save_dir_s = (&mut ws.data.lock().unwrap()).save_dir.to_string_lossy().to_string();
        ctx.text(format!(r#"{{ "event":"set-save-dir", "save-dir": "{}" }}"#, &save_dir_s[..] ));
      }
    }
    else if json["event"] == json!("pick-savedir") {
      if let Some(save_dir) = gui::fork_ask_for_dir() {
        ctx.text(format!(r#"{{ "event":"set-save-dir", "save-dir": "{}" }}"#, &save_dir.to_string_lossy() ));
        (&mut ws.data.lock().unwrap()).save_dir = save_dir;
      }
    }
  }

  println!("handle_ws_msg DONE");

}

// This fn upgrades /ws/ http requests to a websocket connection
// which may stream events to/from the GUI
async fn ws_handler(req: HttpRequest, stream: web::Payload, data: web::Data<Mutex<GlobalData>>) -> Result<HttpResponse, Error> {
    let mut apod_ws = APodWs::new(data);
    if let Some(addr) = req.peer_addr() {
      apod_ws.is_leader = addr.ip().is_loopback();
    }
    let resp = ws::start(apod_ws, &req, stream);
    //println!("{:?}", resp);
    resp
}

// This fn grabs assets and returns them
fn index(req: HttpRequest, _stream: web::Payload) -> HttpResponse {
  
  // We perform some common routing tactics here
  let mut r_path = req.path();
  if r_path == "/" {
    r_path = "index.html";
  }
  if r_path.starts_with("/") {
    r_path = &r_path[1..];
  }
  //println!("r_path={}", &r_path);

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

// This expects video/webm data + saves it to the 
fn save(req: HttpRequest, body: web::Bytes, data: web::Data<Mutex<GlobalData>>) -> HttpResponse {
  use std::process::Command;

  let save_num = req.path().replace(|c: char| !c.is_numeric(), "");
  let save_num: usize = save_num.parse().unwrap_or(0);

  let mut save_f = {
    match data.lock() {
      Ok(data) => data.save_dir.clone(),
      Err(e) => {
        println!("e={}", e);
        return HttpResponse::Ok()
          .content_type("text/html")
          .body(&include_bytes!("www/404.html")[..]);
      }
    }
  };
  
  let mut segment = 0;
  save_f.push(format!("video{}_segment{}.webm", save_num, segment).as_str());

  while save_f.as_path().exists() {
    segment += 1;
    save_f.pop();
    save_f.push(format!("video{}_segment{}.webm", save_num, segment).as_str());
  }

  // save_f now does not exist, write this chunk of video to the file.
  
  println!("[save] Saving {} bytes to {}", body.len(), &save_f.to_string_lossy()[..]);

  // Write to temp file and use ffmpeg to merge chunks
  let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(false)
        .open(&save_f)
        .unwrap();

  let mut total_written = 0;
  let mut remaining_retries = 10;
  while total_written < body.len() && remaining_retries > 0 {
    remaining_retries -= 1;
    total_written += match file.write(&body[total_written..]) {
      Ok(num_written) => num_written,
      Err(e) => {
        println!("error writing: {}", e);
        continue;
      }
    }
  }

  if let Err(e) = file.flush() {
    println!("Error flushing: {}", e);
  }

  HttpResponse::Ok()
      .content_type("text/plain")
      .body(r#"Data received!"#)
}

pub fn main() -> Result<(), Box<dyn std::error::Error>>  {

  // Find/Generate an SSL identity
  // EDIT nvm we just use include_str!() to grab a committed SSL key.
  // Bad practices all around i know.

  let sys = actix_rt::System::new(crate::APP_NAME);
  
  let address = format!("0.0.0.0:{}", crate::HTTP_PORT);

  let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
  
  let private_k = PKey::from_rsa(
    Rsa::private_key_from_pem(include_bytes!("../ssl/key.pem")).unwrap()
  ).unwrap();
  let cert = openssl::x509::X509::from_pem(
    include_bytes!("../ssl/cert.pem")
  ).unwrap();

  builder
    .set_private_key(&private_k)
    .unwrap();
  builder
    .set_certificate(&cert)
    .unwrap();

  HttpServer::new(||
      App::new()
        .app_data( web::Data::new( Mutex::new( GlobalData::default() ) ) )
        .data(web::PayloadConfig::new(15000000)) // allow 15mb data sent to us
        .route("/ws", web::get().to(ws_handler))
        .route("/save", web::post().to(save))
        // We just hard-code 9 participant endpoints; this could be done better I'm sure.
          .route("/save/0", web::post().to(save))
          .route("/save/1", web::post().to(save))
          .route("/save/2", web::post().to(save))
          .route("/save/3", web::post().to(save))
          .route("/save/4", web::post().to(save))
          .route("/save/5", web::post().to(save))
          .route("/save/6", web::post().to(save))
          .route("/save/7", web::post().to(save))
          .route("/save/8", web::post().to(save))
          .route("/save/9", web::post().to(save))
        .route("/", web::get().to(index))
        .default_service(
          web::route().to(index)
        )

    )
    .workers(1)
    .backlog(16)
    .bind_openssl(&address, builder)?
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


