import ReconnectingWebSocket from 'reconnecting-websocket';
 
import { Elm } from './Main.elm';

document.addEventListener("DOMContentLoaded", function(){
  var app = Elm.Main.init({
    node: document.getElementById('view')
  });
  
  const url = process.env.WS_URL;
  console.log("Connecting to " + url + " ...");
  var socket = new ReconnectingWebSocket(url);
  
  app.ports.sendMessage.subscribe(function(message) {
      socket.send(message);
  });
  
  socket.addEventListener("message", function(event) {
    app.ports.messageReceiver.send(event.data);
  });  
});
