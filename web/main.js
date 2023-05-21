// Setup for the game
import init, { AldonHtmlCanvasGame, aldon_debug_logs } from "./pkg/aldonlib.js";
import "./menu.js";
import {
  Dialog,
  aldonPickButtonDialog,
  aldonInventoryDialog,
  aldonPickupDialog,
  aldonBuySellDialog,
  aldonSpellBookDialog,
} from "./dialog.js";

let game = null;
let unrecoverableError = null;

// TODO: move most or all of this to AldonHtmlCanvasGame
class Game {
  constructor(root, canvas, spritesheet) {
    this.root = root;
    this.canvas = canvas;
    this.spritesheet = spritesheet;
    this.handlingInput = false;
    this.map_id = 0;
    this.mouseX = 0;
    this.mouseY = 0;
    // TODO: properly handle resize by not requiring dialogs to read these
    this.width = 430 * 2;
    this.height = 430 * 2;
    this.dialog = new Dialog(
      root,
      spritesheet,
      this.width,
      this.height,
      0, // margin
    );
    this.is_setup = false;
  }

  setup() {
    const tellMessage = this.dialog.tellMessage.bind(this.dialog);

    // so this doesn't run within game.update and borrow game twice
    const executeTrade = (actor_id, items, buttons) =>
      setTimeout(
        // () => this.dialog.transaction(actor_id, items, buttons),
        () => {
          const dialog = aldonInventoryDialog(this, actor_id, items);
          let div = document.getElementById("game");
          div.appendChild(dialog);
        },
        100,
      );

    const showStats = (stats) => this.dialog.stats(stats);
    const pickButton = (button_idx, buttons) => {
      const dialog = aldonPickButtonDialog(button_idx, buttons, game);
      let div = document.getElementById("game");
      div.appendChild(dialog);
    };
    const pickup = (actorID, items) => {
      setTimeout(() => {
        const dialog = aldonPickupDialog(this, actorID, items);
        let div = document.getElementById("game");
        div.appendChild(dialog);
      }, 100);
    };
    const buysell = (actorID, items, kind) => {
      setTimeout(() => {
        const dialog = aldonBuySellDialog(this, actorID, items, kind);
        let div = document.getElementById("game");
        div.appendChild(dialog);
      }, 100);
    };
    const spellbook = (spells) => {
      setTimeout(() => {
        const dialog = aldonSpellBookDialog(this, spells);
        document.getElementById("game").appendChild(dialog);
      }, 100);
    };

    this.game = new AldonHtmlCanvasGame(
      canvas,
      spritesheet,
      tellMessage,
      executeTrade,
      pickup,
      buysell,
      showStats,
      pickButton,
      spellbook,
      //this.dialog.stats.bind(this.dialog),
    );
    const scale = this.getScale();
    this.setScale(scale);
    this.dialog.setGame(this.game);

    this.root.onmousedown = (e) => {
      this.mouseX = e.offsetX;
      this.mouseY = e.offsetY;
      this.startInputHandling();
    };
    this.root.onmousemove = (e) => {
      this.mouseX = e.offsetX;
      this.mouseY = e.offsetY;
    };
    this.root.onmouseup = () => {
      this.stopInputHandling();
    };
    this.root.ontouchstart = (e) => {
      const menu = document.getElementsByClassName("menu-container")[0];
      if (e.target.id !== "canvas") {
        return;
      }
      const touch = e.touches.item(0);
      const rect = e.target.getBoundingClientRect();
      this.mouseX = touch.pageX - rect.left;
      this.mouseY = touch.clientY - rect.top;

      this.startInputHandling();

      e.preventDefault(); // block mouse events
    };
    this.root.ontouchmove = (e) => {
      if (e.target.id !== "canvas") {
        return;
      }
      const touch = e.touches.item(0);
      const rect = e.target.getBoundingClientRect();
      this.mouseX = touch.pageX - rect.left;
      this.mouseY = touch.clientY - rect.top;

      e.preventDefault(); // block mouse events
    };
    this.root.addEventListener("touchend", (e) => {
      if (e.target.id !== "canvas") {
        return;
      }
      this.stopInputHandling();

      e.preventDefault(); // block mouse events
    });
    this.root.ontouchcancel = (e) => {
      if (e.target.id !== "canvas") {
        return;
      }
      this.stopInputHandling();

      e.preventDefault(); // block mouse events
    };
    this.is_setup = true;
  }

  saveLoad() {
    this.game.save_load();
  }

  update(now) {
    this.game.update(now);
    this.game.render(now);
  }

  loadNextMap() {
    console.clear();
    this.map_id++;
    console.log(`load map ${this.map_id}`);
    this.game.load_map(this.map_id);
  }

  loadMap(id) {
    this.map_id = id;
    console.log(`load map ${id}`);
    this.game.load_map(id);
  }

  startInputHandling(e) {
    if (this.dialog.isOpen()) {
      return;
    }
    this.handlingInput = true;
    let input_handler = () => {
      let dialogs = document.getElementsByTagName("aldon-dialog");
      if (dialogs.length > 0) {
        return;
      }
      // TODO: remove this when all dialogs migrated to aldon-dialog
      dialogs = document.getElementsByClassName("window");
      if (dialogs.length > 0) {
        return;
      }
      if (!this.handlingInput) {
        return;
      }
      const x = this.mouseX / this.getScale();
      const y = this.mouseY / this.getScale();
      this.game.input_down(
        this.mouseX / this.getScale(),
        this.mouseY / this.getScale(),
      );
      setTimeout(input_handler, 250);
    };
    // On android there is some noise when a touch first happens. Wait a
    // little to avoid using this data.
    //setTimeout(input_handler, 1000);
    input_handler();
  }

  stopInputHandling() {
    this.handlingInput = false;

    let dialogs = document.getElementsByTagName("aldon-dialog");
    if (dialogs.length > 0) {
      return;
    }
    // TODO: remove this when all dialogs migrated to aldon-dialog
    dialogs = document.getElementsByClassName("window");
    if (dialogs.length > 0) {
      return;
    }
    this.game.input_up(
      this.mouseX / this.getScale(),
      this.mouseY / this.getScale(),
    );
  }

  getSaveStore() {
    const games = localStorage.getItem("aldon-games");
    if (games === null) {
      return {
        quicksave: null,
        saves: [
          emptySave(),
          emptySave(),
          emptySave(),
          emptySave(),
          emptySave(),
        ],
      };
    }
    return JSON.parse(games);
  }

  putSaveStore(store) {
    store.saves.sort((a, b) => {
      if (a.time === null) {
        return 1;
      }
      if (b.time === null) {
        return -1;
      }
      if (a.time < b.time) {
        return 1;
      }
      if (a.time > b.time) {
        return -1;
      }
      return 0;
    });
    const payload = JSON.stringify(store);
    localStorage.setItem("aldon-games", payload);
  }

  quicksave() {
    const store = this.getSaveStore();
    const name = "QuickSave";
    const time = new Date();
    store.quicksave = this.saveImpl(name, time);
    this.putSaveStore(store);
    this.game.log(`*Done Saving ${name} ${time.toLocaleString()}*`);
  }

  deleteQuicksave() {
    const store = this.getSaveStore();
    store.quicksave = null;
    this.putSaveStore(store);
  }

  save(slot) {
    const store = this.getSaveStore();
    const name = this.game.name();
    const time = new Date();
    store.saves[slot] = this.saveImpl(name, time);
    this.putSaveStore(store);
    this.game.log(`*Done Saving ${name} ${time.toLocaleString()}*`);
  }

  loadSave(save) {
    const array = save.data.split(",").map(Number);
    const bytes = new Uint8Array(array);
    this.game.load_save(bytes);
  }

  deleteSave(save) {
    const store = this.getSaveStore();
    for (let i = 0; i < store.saves.length; i++) {
      if (
        store.saves[i].name === save.name &&
        store.saves[i].time == save.time
      ) {
        store.saves[i] = emptySave();
        break;
      }
    }
    this.putSaveStore(store);
  }

  saveImpl(name, time) {
    const save = {
      name,
      time: time.toString(),
      data: this.game.save().toString(),
    };
    return save;
  }

  untoggleMenuButton() {
    this.game.untoggle_menu_button();
  }

  setButton(button_idx, button) {
    this.game.set_button(button_idx, button);
  }

  getLogs() {
    return aldon_debug_logs();
  }

  getScale() {
    const scale = Number(localStorage.getItem("aldon-game-scale")) || 2;
    return Math.min(Math.max(scale, 1), 10);
  }

  setScale(scale) {
    const bounded = Math.min(Math.max(scale, 1), 10);
    localStorage.setItem("aldon-game-scale", bounded);
    // const ctx = this.canvas.getContext("2d");
    // ctx.setTransform(scale, 0, 0, scale, 0, 0);
    this.game.set_scale(bounded);
    this.resize();
    return bounded;
  }

  /** Resize the game to fit the current window */
  resize() {
    let scale = this.getScale();
    this.width = Math.min(430 * scale, window.innerWidth);
    const menu = document.querySelector("#aldon-menu-container");
    let menuHeight = menu.offsetHeight;
    this.height = Math.min(430 * scale, window.innerHeight - menuHeight);
    //menu.style.width = `${this.width - 12}px`;

    const canvas = document.getElementById("canvas");
    canvas.width = this.width;
    canvas.height = this.height;

    const root = document.getElementById("game");
    root.style.width = `${this.width}px`;
    root.style.height = `${this.height}px`;
    this.dialog.resize(this.width, this.height);
  }

  quests() {
    const quests = [];
    for (const quest of this.game.quests()) {
      quests.push({ name: quest });
    }
    return quests;
  }

  showStats(actorId) {
    this.game.show_stats(actorId);
  }

  playing() {
    if (this.game === null) {
      return false;
    }
    return this.game.playing();
  }
}

function emptySave() {
  return {
    name: "Empty",
    data: "",
    time: null,
  };
}

function showUnrecoverableError(error) {
  unrecoverableError = error;
  document.querySelector("#game").style.visibility = "hidden";
  const gameError = document.querySelector("#game-error");
  gameError.style.visibility = "visible";
  gameError.innerText = `${error}`;
}

function updateLoop(now) {
  if (unrecoverableError !== null) {
    return;
  }
  window.requestAnimationFrame(updateLoop);

  if (game === null || !game.is_setup) {
    const canvas = document.getElementById("canvas");
    const ctx = canvas.getContext("2d");
    ctx.fillStyle = "white";
    ctx.font = "48px PalmOS";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    let animation_idx = Math.floor(now / 400) % 4;
    let msg = "Loading. Please wait" + ".".repeat(animation_idx);
    ctx.fillStyle = "black";
    ctx.fillText(msg, 10, 50);
    return;
  }
  game.update(now);
}

async function createGame() {
  window.js_panic = showUnrecoverableError;

  window.requestAnimationFrame(updateLoop);
  const menu = document.getElementsByTagName("aldon-menu")[0];
  const canvas = document.getElementById("canvas");
  const root = document.getElementById("game");

  const spritesheet = document.getElementById("spritesheet");
  game = new Game(root, canvas, spritesheet);
  // TODO: hack needed so dialogs have access
  window.game = game;
  menu.game = game;

  await init();
  await document.fonts.ready;
  game.setup();
  window.addEventListener("resize", async () => {
    game.resize();
  });

  // TODO: does this actually work?
  new ResizeObserver(() => game.resize()).observe(menu);
}

window.onload = function () {
  "use strict";

  // TODO: this doesn't really work to cache files
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("./sw.js");
  }
  const gameDiv = document.getElementById("game");
  gameDiv.addEventListener("touchend", () => {
    const menu = document.getElementsByTagName("aldon-menu")[0];
    menu.close();
  });

  const startBtn = document.getElementById("start-btn");
  const gameContainer = document.getElementById("game-container");

  const displayGame = () => {
    gameContainer.style.display = "";
    startBtn.style.display = "none";
    if (game === null) {
      createGame();
    }
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
  gameContainer.onfullscreenchange = (ev) => {
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
