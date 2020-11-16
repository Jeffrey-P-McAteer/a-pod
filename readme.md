
# A-Pod

"Adam's Podcasting Net"

A relatively simple way to get video + audio
from remote parties onto your own system.

# Building

Use [rustup](https://rustup.rs/) to install a cargo toolchain (or your OS's package manager).

```bash
cargo build --release
# Puts executable in target/release/a-pod.exe

# If on a *nix system and you need a windows .exe
cargo build --release --target=x86_64-pc-windows-gnu

# To build macos from *nix
cargo build --release --target=x86_64-apple-darwin
```

# Usage

1. You will need a "leader" machine which has port 9443 open to the public.
2. Run `a-pod.exe`
3. Send "followers" a link to your public IP address port 9443
4. Connected followers will display in the leader's window

# Common bugs/known issues

 - MacOS webcams cannot be viewed using `navigator.mediaDevices.getUserMedia`?
 - MacOS and Windows firewalls will default to preventing apps from binding to public ports.
   A test is being added to tell you if your OS is preventing your followers from using `a-pod.exe`.


# Design Notes

Originally the plan was to do everything over HTTP, but modern
browsers do [not allow access to getUserMedia](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia#Privacy_and_security) unless it is in a secure context.

Because of this constraint we will generate a temporary SSL key
and use that to encrypt traffic; we have no way of knowing in advance what
the end user's IP address is and I don't to burden them with setting up
an SSL identity in advance. The goal is to accept that users will get an SSL
warning message they will have to allow in order to share their cameras with the leader.

To re-generate ssl credentials run:

```bash
openssl req -x509 -newkey rsa:4096 -nodes -keyout ssl/key.pem -out ssl/cert.pem -days 365 -subj '/CN=unknown-cn'
```


