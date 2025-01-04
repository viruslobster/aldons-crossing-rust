// Setup for the game
// import init, { AldonHtmlCanvasGame, aldon_debug_logs } from "./pkg/aldonlib.js";
// import "./menu.js";
// import { Dialog } from "./dialog.js";

window.onload = function () {
  "use strict";

  // TODO: this doesn't really work to cache files
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("./sw.js");
  }

  const startBtn = document.getElementById("start-btn");
  const gameContainer = document.getElementsByTagName("aldon-game")[0];
  console.log(gameContainer);

  const displayGame = () => {
    console.log(gameContainer.style);
    gameContainer.style.display = "";
    startBtn.style.display = "none";
  };
  const hideGame = () => {
    gameContainer.style.display = "none";
    startBtn.style.display = "";
  };
  if (window.matchMedia("(display-mode: standalone)").matches) {
    // Running as installed PWA
    displayGame();
    return;
  }
  // Running in web browser
  gameContainer.onfullscreenchange = () => {
    if (document.fullscreenElement) {
      displayGame();
    } else {
      hideGame();
    }
  };
  startBtn.onclick = () => {
    if (gameContainer.requestFullscreen) {
      gameContainer.requestFullscreen();
    } else {
      displayGame();
    }
  };
};
