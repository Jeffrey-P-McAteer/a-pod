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

  localVideo = document.getElementById('localVideo');
  remoteVideo = document.getElementById('remoteVideo');

  serverConnection = new WebSocket('wss://'+location.hostname+(location.port ? ':'+location.port: '')+'/ws');
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

  if (window.location.toString().indexOf('leader.html') >= 0) {
    // We assume the websocket connects after 3 seconds
    setTimeout(function() {
      serverConnection.send(JSON.stringify({
        'event': 'leader-joined'
      }));
    }, 3200);
  }

  // Ask cloudfare who we are
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

}

function getUserMediaSuccess(stream) {
  localStream = stream;
  localVideo.srcObject = stream;
}

function start(isCaller) {
  peerConnection = new RTCPeerConnection(peerConnectionConfig);
  peerConnection.onicecandidate = gotIceCandidate;
  peerConnection.ontrack = gotRemoteStream;
  peerConnection.addStream(localStream);

  if (isCaller) {
    peerConnection.createOffer().then(createdDescription).catch(errorHandler);
  }
}

function gotMessageFromServer(message) {
  if(!peerConnection) {
    start(false);
  }

  var signal = JSON.parse(message.data);

  if (signal.event) {
    // This is one of ours
    if (signal.event == "lan-ip") {
      var ip = signal['ip'];
      var p = document.getElementById('private_link');
      p.innerHTML = location.protocol+'//'+ip+(location.port ? ':'+location.port: '');
    }
  }

  // Ignore messages from ourself
  if(signal.uuid == uuid) return;

  if(signal.sdp) {
    peerConnection.setRemoteDescription(new RTCSessionDescription(signal.sdp)).then(function() {
      // Only create answers in response to offers
      if(signal.sdp.type == 'offer') {
        peerConnection.createAnswer().then(createdDescription).catch(errorHandler);
      }
    }).catch(errorHandler);
  } else if(signal.ice) {
    peerConnection.addIceCandidate(new RTCIceCandidate(signal.ice)).catch(errorHandler);
  }
}

function gotIceCandidate(event) {
  if(event.candidate != null) {
    serverConnection.send(JSON.stringify({'ice': event.candidate, 'uuid': uuid}));
  }
}

function createdDescription(description) {
  console.log('got description');

  peerConnection.setLocalDescription(description).then(function() {
    serverConnection.send(JSON.stringify({'sdp': peerConnection.localDescription, 'uuid': uuid}));
  }).catch(errorHandler);
}

function gotRemoteStream(event) {
  console.log('got remote stream');
  remoteVideo.srcObject = event.streams[0];
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


