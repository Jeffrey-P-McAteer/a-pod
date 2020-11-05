var localVideo;
var localStream;
var remoteVideo;
var peerConnection;
var uuid;
var serverConnection;

var peerConnectionConfig = {
  'iceServers': [
    {'urls': 'stun:stun.stunprotocol.org:3478'},
    {'urls': 'stun:stun.l.google.com:19302'},
  ]
};

function pageReady() {
  uuid = createUUID();
  window.is_leader = window.location.toString().indexOf('leader.html') >= 0;

  localVideo = document.getElementById('localVideo');
  window.localVideoFrames = [];

  window.remoteVideo = [];
  window.remoteVideoFrames = [];
  window.remoteVideoRecorder = [];
  for (var i=0; i<5; i++) {
    window.remoteVideo[i] = document.getElementById('remoteVideo'+i);
    window.remoteVideoFrames[i] = [];
  }
  window.peerConnection = [];

  // The next joining websocket gets this number, we loop over at maxNum
  window.maxNum = 5;
  window.remoteNum = 0;

  serverConnection = new WebSocket('wss://'+location.hostname+(location.port ? ':'+location.port: '')+'/ws');
  serverConnection.binaryType = "blob";
  serverConnection.onmessage = gotMessageFromServer;

  var constraints = {
    video: true,
    audio: true,
  };

  if(navigator.mediaDevices.getUserMedia) {
    navigator.mediaDevices.getUserMedia(constraints).then(getUserMediaSuccess).catch(errorHandler);
  } else {
    alert('Your browser does not support getUserMedia API');
  }

  if (window.is_leader) {
    // We assume the websocket connects after 2 seconds
    setTimeout(function() {
      serverConnection.send(JSON.stringify({
        'event': 'leader-joined'
      }));
    }, 2400);
  }

  // Ask cloudfare who we are (usually ipv6 responses)
  var x = new XMLHttpRequest();
  x.onreadystatechange = function() { 
      if (x.readyState == 4 && x.status == 200) {
        // TODO this is only valid for ipv4 ranges
        const ipRegex = /[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}/;
        var p = document.getElementById('public_link');
        var matches = x.responseText.match(ipRegex);
        const pub_ip = (matches && matches.length) > 0? matches[0] : "Unknown IP";

        var pub_url = location.protocol+'//'+pub_ip+(location.port ? ':'+location.port: '');
        p.innerHTML = pub_url;
      }
  }
  x.open("GET", 'https://www.cloudflare.com/cdn-cgi/trace', true); // true means asynchronous
  x.send(null);
  // Ask ifconfig.me who we are (usually ipv4 responses)
  var x = new XMLHttpRequest();
  x.onreadystatechange = function() { 
      if (x.readyState == 4 && x.status == 200) {
        // TODO this is only valid for ipv4 ranges
        const ipRegex = /[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}/;
        var p = document.getElementById('public_link');
        var matches = x.responseText.match(ipRegex);
        const pub_ip = (matches && matches.length) > 0? matches[0] : "Unknown IP";

        var pub_url = location.protocol+'//'+pub_ip+(location.port ? ':'+location.port: '');
        p.innerHTML = pub_url;
      }
  }
  x.open("GET", 'https://ifconfig.me/', true); // true means asynchronous
  x.send(null);

  // Periodically poll localVideoFrames and remoteVideoFrames,
  // pushing both to the server as binary data
  setInterval(function() {

    postFramesToServer(0, window.localVideoFrames, window.localVideoRecorder, function() {
      window.localVideoFrames = [];
    });

    postFramesToServer(1, window.remoteVideoFrames[0], window.remoteVideoRecorder[0], function() {
      window.remoteVideoFrames[0] = [];
    });
    postFramesToServer(2, window.remoteVideoFrames[1], window.remoteVideoRecorder[1], function() {
      window.remoteVideoFrames[1] = [];
    });
    // postFramesToServer(3, window.remoteVideoFrames[2], window.remoteVideoRecorder[2], function() {
    //   window.remoteVideoFrames[2] = [];
    // });
    // postFramesToServer(4, window.remoteVideoFrames[3], window.remoteVideoRecorder[3], function() {
    //   window.remoteVideoFrames[3] = [];
    // });
    // postFramesToServer(5, window.remoteVideoFrames[4], window.remoteVideoRecorder[4], function() {
    //   window.remoteVideoFrames[4] = [];
    // });

  }, 5000);

}

function postFramesToServer(num, frames, recorder, clear_data_fn) {
  let recordedBlob = new Blob(frames, { type: "video/webm" });
  
  // Only tx if we have data
  if (recordedBlob.size > 0) {
    //let recordedURL = URL.createObjectURL(recordedBlob);

    console.log(frames, recordedBlob);

    if (serverConnection.readyState == 2 || serverConnection.readyState == 3) {
      console.log('re-opening websocket, serverConnection.readyState=', serverConnection.readyState);
      serverConnection = new WebSocket('wss://'+location.hostname+(location.port ? ':'+location.port: '')+'/ws');
      serverConnection.binaryType = "blob";
      serverConnection.onmessage = gotMessageFromServer;
    }

    //serverConnection.send(recordedBlob);
    // instead POST fragment to /save
    fetch('https://'+location.hostname+(location.port ? ':'+location.port: '')+'/save/'+num, {
      method: 'POST',
      cache: 'no-cache',
      headers: {
        'Content-Type': 'video/webm'
      },
      body: recordedBlob
    });

    // Zero buffer; any chance we could drop frames this way?
    //window.localVideoFrames.splice(0,window.localVideoFrames.length);
    clear_data_fn();

    // I think .stop() then .start() will give us a new webm header,
    // which we need to join 2 fragments together using mkvmerge
    recorder.stop();
    recorder.start();

  }

  // Ask for new data to be written to the buffer
  if (recorder) {
    if (recorder.state != "recording") {
      recorder.start();
    }
    else {
      recorder.requestData();
    }
  }
}

function getUserMediaSuccess(stream) {
  localStream = stream;
  localVideo.srcObject = stream;
  localVideo.captureStream = localVideo.captureStream || localVideo.mozCaptureStream;
  // Save localVideo frames to the localVideoFrames buffer
  window.localVideoRecorder = new MediaRecorder(localVideo.captureStream(), {
    "mimeType": "video/webm"
  });
  window.localVideoRecorder.ondataavailable = function(event) {
      console.log(event);
      window.localVideoFrames.push(event.data);
  };
  // Start 1/4 second after getting video feed, otherwise we get
  // "DOMException: MediaRecorder.start: The MediaStream is inactive"
  setTimeout(function() {
    window.localVideoRecorder.start();
  }, 250);


}

function start(isCaller) {
  window.peerConnection[window.remoteNum] = new RTCPeerConnection(peerConnectionConfig);
  window.peerConnection[window.remoteNum].onicecandidate = gotIceCandidate;
  window.peerConnection[window.remoteNum].ontrack = gotRemoteStream;
  window.peerConnection[window.remoteNum].addStream(localStream);

  if (isCaller) {
    window.peerConnection[window.remoteNum].createOffer().then(createdDescription).catch(errorHandler);
  }
  
}

function gotMessageFromServer(message) {
  if(!window.peerConnection[window.remoteNum]) {
    start(false);
  }

  var signal = JSON.parse(message.data);
  console.log(signal);

  if (signal.event) {
    // This is one of ours
    if (signal.event == "lan-ip") {
      var ip = signal['ip'];
      var p = document.getElementById('private_link');
      p.innerHTML = location.protocol+'//'+ip+(location.port ? ':'+location.port: '');
    }
    else if (signal.event == "set-save-dir") {
      var save_dir = signal['save-dir'];
      var p = document.getElementById('save_dir');
      p.innerHTML = save_dir;
    }
  }

  // Ignore messages from ourself
  if(signal.uuid == uuid) return;

  if(signal.sdp) {
    window.peerConnection[window.remoteNum].setRemoteDescription(new RTCSessionDescription(signal.sdp)).then(function() {
      // Only create answers in response to offers
      if(signal.sdp.type == 'offer') {
        window.peerConnection[window.remoteNum].createAnswer().then(createdDescription).catch(errorHandler);
      }
    }).catch(errorHandler);
  } else if(signal.ice) {
    window.peerConnection[window.remoteNum].addIceCandidate(new RTCIceCandidate(signal.ice)).catch(errorHandler);
  }
}

function gotIceCandidate(event) {
  if(event.candidate != null) {
    serverConnection.send(JSON.stringify({'ice': event.candidate, 'uuid': uuid}));
  }
}

function createdDescription(description) {
  console.log('got description');

  window.peerConnection[window.remoteNum].setLocalDescription(description).then(function() {
    serverConnection.send(JSON.stringify({'sdp': window.peerConnection[window.remoteNum].localDescription, 'uuid': uuid}));
  }).catch(errorHandler);
}

function gotRemoteStream(event) {
  console.log('got remote stream');
  if (window.remoteNum >= window.maxNum) {
    window.remoteNum = 0;
  }

  remoteVideo[window.remoteNum].srcObject = event.streams[0];
  remoteVideo[window.remoteNum].captureStream = remoteVideo[window.remoteNum].captureStream || remoteVideo[window.remoteNum].mozCaptureStream;

  window.remoteVideoRecorder[window.remoteNum] = new MediaRecorder(remoteVideo[window.remoteNum].captureStream(), {
    "mimeType": "video/webm"
  });
  window.remoteVideoRecorder[window.remoteNum].ondataavailable = function(event) {
      console.log(event);
      window.remoteVideoFrames.push(event.data);
  };
  // Start 1/4 second after getting video feed, otherwise we get
  // "DOMException: MediaRecorder.start: The MediaStream is inactive"
  setTimeout(function() {
    window.remoteVideoRecorder[window.remoteNum].start();
  }, 250);

  window.remoteNum += 1;

}

function errorHandler(error) {
  console.log(error);
}

// Taken from http://stackoverflow.com/a/105074/515584
// Strictly speaking, it's not a real UUID, but it gets the job done here
function createUUID() {
  function s4() {
    return Math.floor((1 + Math.random()) * 0x10000).toString(16).substring(1);
  }

  return s4() + s4() + '-' + s4() + '-' + s4() + '-' + s4() + '-' + s4() + s4() + s4();
}

function pickSaveDir() {
  serverConnection.send(JSON.stringify({
    'event': 'pick-savedir'
  }));
}

