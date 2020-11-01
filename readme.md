
# A-Pod

"Adam's Podcasting Net"

A relatively simple way to get video + audio
from remote parties onto your own system.

# Building

```bash
cargo build --release
# Puts executable in target/release/a-pod.exe

# If on a *nix system and you need a windows .exe
cargo build --release --target=x86_64-pc-windows-gnu
```

# Usage

1. You will need a "leader" machine which has port 8080 open to the public.
2. Run `a-pod.exe`
3. Send "followers" a link to your public IP address port 8080
4. Connected followers will display in the leader's window



