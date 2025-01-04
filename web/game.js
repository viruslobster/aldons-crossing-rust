// Setup for the game
import init, { AldonHtmlCanvasGame, aldon_debug_logs } from "./pkg/aldonlib.js";
import "./menu.js";
import { Dialog } from "./dialog.js";

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
    this.dialog = new Dialog(root);
    this.is_setup = false;
  }

  get width() {
    // TODO: this isn't ideal, should be able to just read from the real source
    const width = this.root.style.getPropertyValue("width").replace(/px/, "");
    return parseInt(width);
  }

  get height() {
    // TODO: this isn't ideal, should be able to just read from the real source
    const height = this.root.style.getPropertyValue("height").replace(/px/, "");
    return parseInt(height);
  }

  setup() {
    this.game = new AldonHtmlCanvasGame(
      this.canvas,
      this.spritesheet,
      this.dialog,
    );
    const scale = this.getScale();
    this.setScale(scale);
    this.dialog.setGame(this);

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

  update(now) {
    this.game.update(now);
    this.game.render(now);
  }

  startInputHandling() {
    if (this.dialog.isOpen()) {
      return;
    }
    this.handlingInput = true;
    let input_handler = () => {
      if (!this.handlingInput || this.dialog.isOpen()) {
        return;
      }
      this.game.input_down(
        this.mouseX / this.getScale(),
        this.mouseY / this.getScale(),
      );
      setTimeout(input_handler, 250);
    };
    input_handler();
  }

  stopInputHandling() {
    this.handlingInput = false;

    if (this.dialog.isOpen()) {
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
    return bounded;
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

/**
 * A simple way to create html inline like html(`<div prop="foo">text</div>`)
 *
 * You might think we can use a <template> instead of a <div> here
 * but that doesn't work if you're trying to use custom elements.
 */
function html(content) {
  const el = document.createElement("div");
  el.innerHTML = content.trim();
  return el.firstChild;
}

class AldonGame extends HTMLElement {
  constructor() {
    super();
    this.unrecoverableError = null;
    this.is_setup = false;
    const style = document.createElement("style");
    style.textContent = `
      html {
      }

      canvas {
        display: block;
        z-index: 1;
        font-smooth: never;
        -webkit-font-smoothing: none;
      }

      button {
        font-family: inherit;
        background-color: white;
        border-radius: 15px;
        border: 4px solid black;
        padding: 0 10px 0 10px;
        margin: 0 10px 0 0;
        font-size: inherit;
        color: black;
      }

      button:active {
        background-color: #2c008b;
        color: white;
      }

      button.decrement {
        margin-right: 0px;
      }

      button.increment {
        margin-left: 0px;
      }

      input {
        font-family: PalmOS;
        font-smooth: never;
        font-size: inherit;
      }

      .stat-row-label {
        display: inline-block;
      }

      .picture {
        position: absolute;
        right: 2px;
        top: 2px;
      }

      .race-container div {
        display: inline-block;
      }

      .stat-row {
        white-space: nowrap;
      }

      .button-container {
        position: absolute;
        bottom: 10px;
        right: 10px;
        font-size: 100px;
      }
      
      .responses {
        position: absolute;
        bottom: 10px;
        left: 10px;
        font-size: 100px;
      }

      #canvas {
        position: absolute;
      }

      #game-container {
        background: black;
        position: relative;
        line-height: normal;
        letter-spacing: normal;
      }

      #game-error {
        color: red;
        visibility: hidden;
        font-size: 20px;
      }
    `;

    let spritesheetUrl = new URL("/assets/spritesheet.png", import.meta.url);
    const root = html(`
      <div id="game-container">
        <!-- On safari the height of a custom element can't always be measured, 
             so wrap aldon-menu in another div -->
        <div id="aldon-menu-container">
          <aldon-menu></aldon-menu>
        </div>
        <div id="game">
          <canvas id="canvas"></canvas>
          <div id="game-error"></div>
        </div>
        <img id="spritesheet" src="${spritesheetUrl}" style="display: none" />
      </div>
    `);

    const game = root.querySelector("#game");
    game.addEventListener("touchend", () => {
      const menu = root.getElementsByTagName("aldon-menu")[0];
      menu.close();
    });

    const shadowRoot = this.attachShadow({ mode: "open" });
    shadowRoot.appendChild(style);
    shadowRoot.appendChild(root);
  }

  async connectedCallback() {
    window.js_panic = () => this.showUnrecoverableError();

    window.requestAnimationFrame((now) => this.updateLoop(now));
    const menu = this.shadowRoot.querySelector("aldon-menu");
    const canvas = this.shadowRoot.querySelector("#canvas");
    const root = this.shadowRoot.querySelector("#game");
    const spritesheet = this.shadowRoot.querySelector("#spritesheet");

    this.game = new Game(root, canvas, spritesheet);
    // TODO: hack needed so dialogs have access
    window.game = this.game;
    menu.game = this.game;

    await init();
    await document.fonts.ready;
    this.game.setup();
    menu.dialog = this.game.dialog;
    window.addEventListener("resize", async () => {
      this.resize();
    });
    this.resize();
    this.is_setup = true;

    // TODO: does this actually work?
    // new ResizeObserver(() => this.game.resize()).observe(menu);
  }

  resize() {
    // TODO: all of this can probably be replaced with some simple css

    let parentWidth = this.parentNode.offsetWidth;
    let parentHeight = this.parentNode.offsetHeight;
    let scale = this.game.getScale();
    this.width = Math.min(430 * scale, parentWidth);
    const menu = this.shadowRoot.querySelector("#aldon-menu-container");
    let menuHeight = menu.offsetHeight;
    this.height = Math.min(430 * scale, parentHeight - menuHeight);

    const canvas = this.shadowRoot.querySelector("#canvas");
    canvas.width = this.width;
    canvas.height = this.height;

    const root = this.shadowRoot.querySelector("#game");
    root.style.width = `${this.width}px`;
    root.style.height = `${this.height}px`;
  }

  updateLoop(now) {
    if (this.unrecoverableError !== null) {
      return;
    }
    window.requestAnimationFrame((now) => this.updateLoop(now));

    if (!this.game || !this.is_setup) {
      const canvas = this.shadowRoot.querySelector("#canvas");
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
    this.game.update(now);
  }

  showUnrecoverableError(error) {
    this.unrecoverableError = error;
    this.shadowRoot.querySelector("#game").style.visibility = "hidden";
    const gameError = this.shadowRoot.querySelector("#game-error");
    gameError.style.visibility = "visible";
    gameError.innerText = `${error}`;
  }

  // TODO: remove when you get rid of old style dialogs
  closeWindow() {
    const window = this.shadowRoot.querySelector("#window");
    if (window == null) throw "could not find window element";
    window.remove();
  }
}
customElements.define("aldon-game", AldonGame);
