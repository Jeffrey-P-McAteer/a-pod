
use std::env;

mod webserver;
mod gui;

// These constants may be read from anywhere in
// the program and change default behaviours
pub const HTTP_PORT: u64 = 9443;
pub const APP_NAME: &'static str = "A-Pod";

fn main() {
  // Process sub-"functions" which must be their own procs
  // for stupid runtime/gtk/win32 threading reasons.
  if let Some(arg) = env::args().skip(1).next() {
    let arg = &arg[..];
    if arg == "ask-for-dir" {
      let d = gui::ask_for_directory();
      println!("{}", d);
      return;
    }
  }

  // Setup event handler for OS signals
  let e = ctrlc::set_handler(move || {
    std::process::exit(0);
  });
  if let Err(e) = e {
    println!("Error setting signal handler: {}", e);
  }

  // Run background threads in the background
  std::thread::spawn(bg_main);

  // Run graphics on main thread (windows cares quite a bit about this)
  if let Err(e) = gui::main() {
    println!("gui error = {:?}", e);
    std::process::exit(1);
  }
}

fn bg_main() {
  crossbeam::thread::scope(|s| {
    
    s.spawn(|_| {
      if let Err(e) = webserver::main() {
        println!("webserver error = {:?}", e);
        std::process::exit(1);
      }
    });

  }).expect("Error joining threads");
}


