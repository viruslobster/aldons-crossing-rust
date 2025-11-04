// Game dialogs like inventory, buy/sell, stats, etc. My aim was to depend
// on no third party js. There are two attempts to do this still here. The
// first is a mess and I want to rewrite them. The second is slightly better
// and uses native web components.

import { Stats } from "./pkg/aldonlib.js";

// TODO: don't duplicate all this information here just so we can draw portraits. Instead expose a drawing function from rust.
const portraits = [
  { id: 600, x: 235, y: 91 },
  { id: 601, x: 269, y: 91 },
  { id: 602, x: 303, y: 91 },
  { id: 603, x: 337, y: 91 },
  { id: 604, x: 371, y: 91 },
  { id: 605, x: 405, y: 91 },
  { id: 606, x: 439, y: 91 },
  { id: 607, x: 473, y: 91 },
  { id: 650, x: 507, y: 91 },
  { id: 651, x: 541, y: 91 },
  { id: 652, x: 575, y: 91 },
  { id: 653, x: 1, y: 125 },
  { id: 654, x: 35, y: 125 },
  { id: 655, x: 69, y: 125 },
  { id: 656, x: 103, y: 125 },
  { id: 657, x: 137, y: 125 },
  { id: 700, x: 171, y: 125 },
  { id: 704, x: 307, y: 125 },
  { id: 707, x: 409, y: 125 },
  { id: 708, x: 443, y: 125 },
  { id: 709, x: 477, y: 125 },
  { id: 710, x: 511, y: 125 },
  { id: 712, x: 1, y: 159 },
  { id: 713, x: 35, y: 159 },
  { id: 714, x: 69, y: 159 },
  { id: 715, x: 103, y: 159 },
  { id: 715, x: 137, y: 159 },
];

const USER_PORTRAITS = [
  { id: 600, x: 235, y: 91 },
  { id: 601, x: 269, y: 91 },
  { id: 602, x: 303, y: 91 },
  { id: 603, x: 337, y: 91 },
  { id: 604, x: 371, y: 91 },
  { id: 605, x: 405, y: 91 },
  { id: 606, x: 439, y: 91 },
  { id: 607, x: 473, y: 91 },
  { id: 650, x: 507, y: 91 },
  { id: 651, x: 541, y: 91 },
  { id: 652, x: 575, y: 91 },
  { id: 653, x: 1, y: 125 },
  { id: 654, x: 35, y: 125 },
  { id: 655, x: 69, y: 125 },
  { id: 656, x: 103, y: 125 },
  { id: 657, x: 137, y: 125 },
];

const RACES = ["human", "dwarf", "elf"];

const DEFAULT_STATS = {
  human: {
    str: 8,
    dex: 8,
    vit: 8,
    int: 8,
    wis: 8,
    luck: 8,
  },
  dwarf: {
    str: 10,
    dex: 7,
    vit: 9,
    int: 7,
    wis: 8,
    luck: 8,
  },
  elf: {
    str: 7,
    dex: 10,
    vit: 7,
    int: 8,
    wis: 8,
    luck: 8,
  },
};

const STAT_BOUNDS = {
  human: {
    str: [3, 16],
    dex: [3, 16],
    vit: [3, 16],
    int: [3, 16],
    wis: [3, 16],
    luck: [3, 18],
  },
  dwarf: {
    str: [5, 17],
    dex: [2, 15],
    vit: [5, 17],
    int: [2, 15],
    wis: [3, 16],
    luck: [3, 16],
  },
  elf: {
    str: [2, 15],
    dex: [5, 18],
    vit: [2, 15],
    int: [3, 16],
    wis: [3, 16],
    luck: [3, 16],
  },
};

class AldonPicker extends HTMLElement {
  constructor() {
    super();
    this.items = [];
    this.getGold = () => 0;
    this.itemNamer = (item, _idx) => item.name;
    this.updateListeners = [];

    const style = document.createElement("style");
    style.textContent = `
      .root {
        font-size: 80px;
        touch-action: pan-y;
      }

      .table {
        height: 4em;
        overflow: auto;
        margin: 10px;
        border: 1px solid black;
        touch-action: pan-y;
      }

      .table .selected {
        color: white;
        background-color: black;
        touch-action: pan-y;
      }

      .info-container {
        display: inline-block;
      }

      .info-container canvas {
        display: inline-block;
        margin: 10px;
      }

      .info-text {
        float: inline-end;
        display: inline-block;
      }

    `;
    const preview = this.getAttribute("previewPic") === "true";
    const info = this.getAttribute("itemInfo") === "true";
    const root = html(`<div class="root"></div>`);

    if (info) {
      const div = html(`<div class="restriction"></div>`);
      root.appendChild(div);
    }
    const table = html(`<div class="table"></div>`);
    root.appendChild(table);

    const infoContainer = html(`<span class="info-container"></span>`);

    if (preview) {
      const scale = 8;
      const canvas = html(`
        <canvas class="preview" width="${20 * scale}" height="${20 * scale}"></canvas>
      `);
      const ctx = canvas.getContext("2d");
      ctx.imageSmoothingEnabled = false;
      ctx.scale(scale, scale);
      this.addEventListener("click", () => this.updatePreview());
      infoContainer.appendChild(canvas);
    }
    if (info) {
      const div = html(`
        <div class="info-text">
          <div class="info"></div>
          <div class="gold">Gold:</div>
        </div>
      `);
      infoContainer.appendChild(div);
    }
    if (preview || info) {
      root.appendChild(infoContainer);
    }
    const shadowRoot = this.attachShadow({ mode: "open" });
    shadowRoot.appendChild(style);
    shadowRoot.appendChild(root);
  }

  connectedCallback() {}

  /**
   * item: { name: string }
   */
  addItem(item) {
    const div = document.createElement("div");
    this.items.push(item);
    div.innerHTML = `${this.itemNamer(item, this.items.length - 1)}`;
    if (this.items.length == 1) {
      div.classList.add("selected");
    }

    div.addEventListener("click", (e) => {
      const selected = this.shadowRoot.querySelector(".selected");
      selected.classList.remove("selected");
      e.target.classList.add("selected");
      this.update();
    });

    const container = this.shadowRoot.querySelector(".table");
    container.appendChild(div);
    this.update();
  }

  /* Returns the index of the next selected item after the currently
   * selected item is removed.
   */
  nextIndex() {
    const selected = this.shadowRoot.querySelector(".selected");
    if (selected === null) {
      return null;
    }
    if (this.items.length <= 1) {
      return null;
    }
    const divs = this.shadowRoot.querySelector(".table").children;
    const i = [...divs].indexOf(selected);

    if (i === this.items.length - 1) {
      return i - 1;
    }
    return i;
  }

  selected() {
    const i = this.selectedIndex();
    if (i === null) {
      return null;
    }
    return this.items[i];
  }

  selectedIndex() {
    const selected = this.shadowRoot.querySelector(".selected");
    if (selected === null) {
      return null;
    }
    const divs = this.shadowRoot.querySelector(".table").children;
    const i = [...divs].indexOf(selected);
    return i;
  }

  removeSelected() {
    const selected = this.shadowRoot.querySelector(".selected");
    if (selected === null) {
      return;
    }
    const selectedIdx = this.selectedIndex();
    const newIdx = this.nextIndex();

    selected.classList.remove("selected");
    selected.remove();
    this.items.splice(selectedIdx, 1);

    if (newIdx === null) {
      return;
    }
    const divs = this.shadowRoot.querySelector(".table").children;
    const newSelected = divs[newIdx];
    newSelected.classList.add("selected");
    this.update();
  }

  update(message) {
    for (const listener of this.updateListeners) {
      listener();
    }
    const itemDivs = this.shadowRoot.querySelector(".table").children;
    for (let i = 0; i < this.items.length; i++) {
      itemDivs[i].innerText = this.itemNamer(this.items[i], i);
    }
    this.updatePreview();
    if (this.getAttribute("itemInfo") !== "true") {
      return;
    }
    const selected = this.selected();
    if (selected === null) {
      return;
    }
    const restriction = this.shadowRoot.querySelector(".restriction");
    restriction.innerText = message || selected.restriction;

    const info = this.shadowRoot.querySelector(".info");
    info.innerText = selected.info;

    const gold = this.shadowRoot.querySelector(".gold");
    gold.innerText = `Gold: ${this.getGold()}`;
  }

  updatePreview() {
    const canvas = this.shadowRoot.querySelector(".preview");
    if (canvas === null) {
      return;
    }
    const ctx = canvas.getContext("2d");

    const item = this.selected();
    if (item === null) {
      return;
    }
    // draw background
    ctx.drawImage(game.spritesheet, 247, 299, 20, 20, 0, 0, 20, 20);
    ctx.drawImage(
      game.spritesheet,
      item.frame.x,
      item.frame.y,
      item.frame.w,
      item.frame.h,
      2,
      2,
      item.frame.w,
      item.frame.h,
    );
  }
}
customElements.define("aldon-picker", AldonPicker);

class AldonDialog extends HTMLElement {
  constructor() {
    super();
    const width = this.getAttribute("width");
    const height = this.getAttribute("height");
    console.log(`width: ${width}, height: ${height}`);
    const scale = (Math.min(width, height) - 10) / 860;
    const border = 3;
    const top = height / 2 - (860 * scale) / 2 - border * scale;
    const left = window.innerWidth / 2 - (860 * scale) / 2 - border * scale;
    const style = document.createElement("style");

    style.textContent = `
      .window {
        width: 860px;
        height: 860px;
        transform: scale(${scale}, ${scale});
        transform-origin: top left;
        top: ${top}px;
        left: ${left}px;
        font-family: PalmOS;
        font-smooth: never;
        font-size: 90px;
        border-radius: 8px;
        position: absolute;
        border: ${border}px solid #2c008b;
        vertical-align: top;
        z-index: 2;
        background-color: white;
        overflow: hidden;
        touch-action: pan-y;
        display: flex;
        flex-direction: column;
      }

      .window .title {
        font-family: PalmOSBold;
        background-color: #2c008b;
        text-align: center;
        color: white;
        border: 5px solid #2c008b;
        height: 10%;
      }

      .window .body {
        font-family: PalmOS;
        font-smooth: never;
        padding: 5px;
        touch-action: pan-y;
        height: 90%;
        position: relative;
      }
    `;
    const shadowRoot = this.attachShadow({ mode: "open" });
    shadowRoot.appendChild(style);
    shadowRoot.appendChild(dialogTemplate.content.cloneNode(true));
  }
}
customElements.define("aldon-dialog", AldonDialog);

class AldonDialogDoneButton extends HTMLElement {
  constructor() {
    super();
  }

  connectedCallback() {
    this.innerHTML = `
      <button class="done">Done</button>
    `;

    const btn = this.querySelector(".done");
    btn.addEventListener("click", () => this.close());
  }

  close() {
    const dialog = this.closest("aldon-dialog");
    dialog.remove();
  }
}
customElements.define("aldon-dialog-done-button", AldonDialogDoneButton);

class CreateCharacterDialog extends HTMLElement {
  constructor() {
    super();
    this.game = null;
    this.playerName = "";
    this.picIdx = 0;
    this.raceIdx = 0;
    this.stats = { ...DEFAULT_STATS[this.race] };
    this.points = 12;
  }

  get race() {
    return RACES[this.raceIdx];
  }

  connectedCallback() {
    this.render();
  }

  render() {
    const width = this.getAttribute("width");
    const height = this.getAttribute("height");
    const raceName = capitalize(this.race);
    const scale = 5;
    this.innerHTML = `
      <aldon-dialog width="${width}" height="${height}">
        <div slot="title">Character Creation</div>
        <div slot="body">
          <input
            style="width: 45%"
            class="player-name"
            maxlength="14"
            type="text"
            placehold="Enter Name"
            value="${this.playerName}"
          >
          <canvas class="picture" width="${32 * scale}" height="${32 * scale}"></canvas>
          <button class="prev-pic"><</button>
          <button class="next-pic">></button>
          <div class="race-container">
            <button class="next-race">></button>
            <div>Race:${raceName}</div>
          </div>
          <div>
            ${this.statRow("str")}
            ${this.statRow("dex")}
            ${this.statRow("vit")}
            ${this.statRow("int")}
            ${this.statRow("wis")}
            ${this.statRow("luck")}
          </div>
          <div>Points:${this.points}</div>
          <div class="right-button-container">
            <button class="done">Done</button>
          </div>
        </div>
      </aldon-dialog>
    `;
    const nextRaceBtn = this.querySelector(".next-race");
    nextRaceBtn.onclick = () => this.nextRace();

    for (const row of this.querySelectorAll(".stat-row")) {
      const kind = row.getAttribute("kind");
      row.querySelector(".decrement").onclick = () => this.decrementStat(kind);
      row.querySelector(".increment").onclick = () => this.incrementStat(kind);
    }
    const nameInput = this.querySelector(".player-name");
    nameInput.oninput = () => (this.playerName = nameInput.value);

    const prevPic = this.querySelector(".prev-pic");
    prevPic.onclick = () => this.nextPic(/*backwards*/ true);
    const nextPic = this.querySelector(".next-pic");
    nextPic.onclick = () => this.nextPic();

    const doneBtn = this.querySelector(".done");
    doneBtn.onclick = () => this.done();

    this.renderCanvas(scale);
  }

  nextPic(backwards = false) {
    this.picIdx += backwards ? -1 : 1;
    if (this.picIdx >= USER_PORTRAITS.length) {
      this.picIdx = 0;
    } else if (this.picIdx < 0) {
      this.picIdx = USER_PORTRAITS.length - 1;
    }
    this.render();
  }

  nextRace() {
    this.raceIdx = (this.raceIdx + 1) % RACES.length;
    this.stats = { ...DEFAULT_STATS[this.race] };
    this.points = 12;
    this.render();
  }

  renderCanvas(scale) {
    const canvas = this.querySelector("canvas");
    const ctx = canvas.getContext("2d");
    ctx.imageSmoothingEnabled = false;
    ctx.scale(scale, scale);
    const x = USER_PORTRAITS[this.picIdx].x;
    const y = USER_PORTRAITS[this.picIdx].y;
    ctx.drawImage(this.game.spritesheet, x, y, 32, 32, 0, 0, 32, 32);
  }

  done() {
    const portrait = USER_PORTRAITS[this.picIdx].id;
    this.game.game.new_game(
      this.playerName,
      this.race,
      portrait,
      this.stats.str,
      this.stats.dex,
      this.stats.vit,
      this.stats.int,
      this.stats.wis,
      this.stats.luck,
    );
    this.remove();
  }

  statRow(kind) {
    const detail = statDetail(kind, this.stats[kind]);
    const name = capitalize(kind);
    const value = this.stats[kind];
    return `
      <div class="stat-row" kind="${kind}">
        <button class="decrement">-</button>
        <button class="increment">+</button>
        <div class="stat-row-label">${name}:${value} (${detail})</div>
      </div>
    `;
  }

  decrementStat(kind) {
    console.log("decrement");
    const [min, _] = STAT_BOUNDS[this.race][kind];

    if (this.stats[kind] <= min) {
      this.stats[kind] = min;
      return;
    }
    this.stats[kind]--;
    this.points++;
    this.render();
    console.log("finished");
  }

  incrementStat(kind) {
    const [_, max] = STAT_BOUNDS[this.race][kind];

    if (this.stats[kind] >= max) {
      this.stats[kind] = max;
      return;
    }
    if (this.points <= 0) {
      this.points = 0;
      return;
    }
    this.points--;
    this.stats[kind]++;
    this.render();
  }
}
customElements.define("aldon-create-character-dialog", CreateCharacterDialog);

class Dialog {
  constructor(root) {
    this.game = null;
    this.root = root;
  }

  isOpen() {
    return this.root.querySelector("aldon-dialog") !== null;
  }

  setGame(game) {
    this.game = game;
  }

  tell(
    title,
    portrait_x,
    portrait_y,
    portrait_w,
    portrait_h,
    msg,
    choiceA,
    choiceB,
    choiceC,
    fromActor,
  ) {
    const scale = 8;
    const stylePicture = `
        position: relative;
        right: 2px;
        top: 2px;
        float: right;
    `;
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">${title}</div>
        <div slot="body">
          <canvas
            style="${stylePicture}" class="picture" width="${32 * scale}" height="${32 * scale}">
          </canvas>
          <div class="message">
            ${msg.replace(/&/gi, "<br>")}
          </div>
          <div class="responses">
          </div>
        </div>
      </aldon-dialog>
    `);
    const pic = dialog.querySelector(".picture");
    const ctx = pic.getContext("2d");
    ctx.imageSmoothingEnabled = false;
    ctx.scale(scale, scale);
    ctx.drawImage(
      this.game.spritesheet,
      portrait_x,
      portrait_y,
      portrait_w,
      portrait_h,
      0,
      0,
      portrait_w,
      portrait_h,
    );
    const choices = [choiceA, choiceB, choiceC];
    const responses = dialog.querySelector(".responses");
    const buttonNames = ["A", "B", "C"];
    for (let i = 0; i < 3; i++) {
      const response = html(`
        <div>
          <button>${buttonNames[i]}</button>
          ${choices[i]}
        </div>
      `);
      const button = response.querySelector("button");
      button.onclick = () => {
        dialog.remove();
        setTimeout(() => {
          this.game.game.send_response(fromActor, i);
        }, 400);
      };
      if (choices[i] === undefined) {
        response.style.visibility = "hidden";
      }
      responses.appendChild(response);
    }
    this.root.appendChild(dialog);
  }

  createCharacter() {
    const dialog = html(`
      <aldon-create-character-dialog
        width="${this.game.width}"
        height=${this.game.height}"
      >
      </aldon-create-character-dialog>
    `);
    dialog.game = this.game;
    this.root.appendChild(dialog);
  }

  inventory(actorID, items) {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Transaction</div>
        <div slot="body">
          <aldon-picker previewPic="true" itemInfo="true">
          </aldon-picker>
          <div class="left-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
            <button class="drop">Drop</button>
            <button class="action"></button>
            <button class="stats">Stats</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const statsBtn = dialog.querySelector(".stats");
    statsBtn.onclick = () => this.game.showStats(actorID);

    const picker = dialog.querySelector("aldon-picker");
    picker.getGold = () => this.game.game.gold(actorID);
    picker.itemNamer = (item, idx) => {
      const equiped = this.game.game.is_equiped(actorID, idx);
      return equiped ? `${equiped} ${item.name}` : item.name;
    };

    for (let i = 0; i < items.length; i++) {
      picker.addItem(items[i]);
    }
    const dropBtn = dialog.querySelector(".drop");
    dropBtn.onclick = () => {
      console.log("drop!");
      const i = picker.selectedIndex();
      const ok = this.game.game.drop(actorID, i);
      if (ok) {
        picker.removeSelected();

        if (picker.items.length === 0) {
          dialog.remove();
          return;
        }
      }
      const message = ok ? null : "Can't drop";
      picker.update(message);
      draw();
    };
    const equip = () => {
      const i = picker.selectedIndex();
      const ok = this.game.game.equip(actorID, i);
      const message = ok ? null : "Can't equip";
      picker.update(message);
      draw();
    };
    const unequip = () => {
      const i = picker.selectedIndex();
      const ok = this.game.game.unequip(actorID, i);
      const message = ok ? null : "Can't unequip";
      picker.update(message);
      draw();
    };
    const use = () => {
      console.log("use!");
      const i = picker.selectedIndex();
      this.game.game.use_item(actorID, i);
      picker.update("Used.");
      const item = picker.items[i];
      console.log(i);
      console.log(item);
      if (item.quantity === 0) {
        picker.removeSelected();
      }
      if (picker.items.length === 0) {
        dialog.remove();
        return;
      }
      draw();
    };
    const draw = () => {
      const i = picker.selectedIndex();
      const actionBtn = dialog.querySelector(".action");
      const item = picker.items[i];

      if (item.usable) {
        actionBtn.innerText = "Use";
        actionBtn.onclick = use;
        return;
      }
      const isEquiped = this.game.game.is_equiped(actorID, i);
      const actionName = isEquiped ? "Unequip" : "Equip";
      actionBtn.innerText = actionName;
      actionBtn.onclick = isEquiped ? unequip : equip;
    };
    draw();
    picker.addEventListener("click", draw);
    this.root.appendChild(dialog);
  }

  pickup(actorID, items) {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Transaction</div>
        <div slot="body">
          <aldon-picker previewPic="true" itemInfo="true">
          </aldon-picker>
          <div class="left-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
            <button class="pickup">Pickup</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const picker = dialog.querySelector("aldon-picker");
    picker.getGold = () => this.game.game.gold(actorID);

    for (const item of items) {
      picker.addItem(item);
    }

    const pickupBtn = dialog.querySelector(".pickup");
    pickupBtn.onclick = () => {
      const i = picker.selectedIndex();
      const ok = this.game.game.pickup(actorID, i);
      if (!ok) {
        picker.update("You can't pick this up.");
        return;
      }
      picker.removeSelected();
      if (picker.items.length === 0) {
        dialog.remove();
      }
    };
    this.root.appendChild(dialog);
  }

  buySell(actorID, items, kind) {
    const kindStr = kind === "buy" ? "Buy" : "Sell";
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Transaction</div>
        <div slot="body">
          <aldon-picker previewPic="true" itemInfo="true">
          </aldon-picker>
          <div class="left-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
            <button class="buysell">${kindStr}</button>
            <button class="unequip">Unequip</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const picker = dialog.querySelector("aldon-picker");
    picker.getGold = () => this.game.game.gold(actorID);
    const cost = (item) => (kind === "buy" ? item.buy_cost : item.sell_cost);
    picker.itemNamer = (item) => `(${cost(item)})${item.name}`;

    for (const item of items) {
      picker.addItem(item);
    }

    const unequipBtn = dialog.querySelector(".unequip");
    unequipBtn.onclick = () => {
      const i = picker.selectedIndex();
      this.game.game.unequip(actorID, i);
      picker.update("Unequiped");
    };
    picker.updateListeners.push(() => {
      const i = picker.selectedIndex();
      unequipBtn.style.visibility = this.game.game.is_equiped(actorID, i)
        ? "visible"
        : "hidden";
    });
    picker.update();

    const buysellBtn = dialog.querySelector(".buysell");
    const buy = () => {
      const i = picker.selectedIndex();
      const ok = this.game.game.buy(actorID, i);
      if (!ok) {
        picker.update("You can't pick this up.");
        return;
      }
      picker.update("Sold!");
    };
    const sell = () => {
      const i = picker.selectedIndex();
      const equiped = this.game.game.is_equiped(actorID, i);
      if (equiped) {
        picker.update("Item is equiped.");
        return;
      }
      const ok = this.game.game.sell(actorID, i);
      if (!ok) {
        picker.update("You can't sell this.");
        return;
      }
      picker.removeSelected();
      if (picker.items.length === 0) {
        dialog.remove();
        return;
      }
      picker.update("Sold!");
    };
    buysellBtn.onclick = kind === "buy" ? buy : sell;
    this.root.appendChild(dialog);
  }

  pickButton(buttonIdx, buttons) {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Transaction</div>
        <div slot="body">
          Set Item or Ability.
          <aldon-picker previewPic="true">
          </aldon-picker>
          <div class="left-button-container">
            <button class="done">Done</button>
            <button class="set">Set</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const picker = dialog.querySelector("aldon-picker");
    for (const button of buttons) {
      picker.addItem(button);
    }

    const doneBtn = dialog.querySelector(".done");
    doneBtn.onclick = () => dialog.remove();

    const setBtn = dialog.querySelector(".set");
    setBtn.onclick = () => {
      const idx = picker.selectedIndex();
      const button = buttons[idx];
      this.game.setButton(buttonIdx, button);
      dialog.remove();
    };
    this.root.appendChild(dialog);
  }

  stats(playerStats) {
    const scale = 5;
    const gp_scale = 4;
    const style_row_item = "width: 30%; display: inline-block";
    const style_gp = `
      display: inline;
    `;
    const can_level_up = Stats.max_level(playerStats.exp) > playerStats.level;
    const style_done_button = `
        position: fixed;
        bottom: 10px;
        right: 10px;
    `;
    console.log(`stats: ${this.game.width} x ${this.game.height}`);

    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Stats</div>
        <div slot="body">
          <div style="overflow: auto">
            <canvas style="float: right" class="picture" width="${32 * scale}" height="${32 * scale}"></canvas>
            <div>${playerStats.name}</div>
            <div>Class: ${playerStats.klass}</div>
            <div>Race: ${playerStats.race}</div>
          </div>
          <div>
            <span style="${style_row_item}">Lvl: ${playerStats.level}${can_level_up ? "+" : ""}</span>
            <span style="${style_row_item}">HP: ${playerStats.hp}/${playerStats.hp_max}</span>
            <span style="${style_row_item}">AC: ${playerStats.ac}</span>
          </div>
          <div style="display: flex">
            <span style="${style_row_item}">EXP: ${playerStats.exp}</span>
            <span style="${style_row_item}">MP: ${playerStats.mp}/${playerStats.mp_max}</span>
            <span style="width: 30%; display: inline-flex">
              <canvas id="gp-pic" style="${style_gp}" width="${16 * gp_scale}" height="${16 * gp_scale}"></canvas>
              <span style="">: ${playerStats.gp}</span>
            <span>
          </div>
          <div>Str:${playerStats.str} (${statDetail("str", playerStats.str)})</div>
          <div>Dex:${playerStats.dex} (${statDetail("dex", playerStats.dex)})</div>
          <div>Vit:${playerStats.vit} (${statDetail("vit", playerStats.vit)})</div>
          <div>Int:${playerStats.int} (${statDetail("int", playerStats.int)})</div>
          <div>Wis:${playerStats.wis} (${statDetail("wis", playerStats.wis)})</div>
          <div>Luck:${playerStats.luck} (${statDetail("luck", playerStats.luck)})</div>
          <div class="right-button-container" style="${style_done_button}">
            <aldon-dialog-done-button></aldon-dialog-done-button>
          </div>
        </div>
      </aldon-dialog>
    `);
    const ctx = dialog.querySelector(".picture").getContext("2d");
    ctx.imageSmoothingEnabled = false;
    ctx.scale(scale, scale);
    const portrait_idx =
      portraits.findIndex((p) => p.id == playerStats.portrait) || 0;
    const x = portraits[portrait_idx].x;
    const y = portraits[portrait_idx].y;
    ctx.drawImage(this.game.spritesheet, x, y, 32, 32, 0, 0, 32, 32);

    const gp_ctx = dialog.querySelector("#gp-pic").getContext("2d");
    gp_ctx.imageSmoothingEnabled = false;
    gp_ctx.scale(gp_scale, gp_scale);
    gp_ctx.drawImage(this.game.spritesheet, 495, 179, 16, 16, 0, 0, 16, 16);
    this.root.appendChild(dialog);
  }

  spellbook(spells) {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Transaction</div>
        <div slot="body">
          <aldon-picker previewPic="true" itemInfo="true">
          </aldon-picker>
          <div class="left-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
            <button class="set">Set</button>
          </div>
        </div>
      </aldon-dialog>
    `);
    const picker = dialog.querySelector("aldon-picker");

    for (const spell of spells) {
      picker.addItem({
        name: spell.name,
        restriction: "Set Item or Ability.",
        info: `Level=${spell.level}, Mana Cost=${spell.cost}`,
        frame: spell.frame,
        id: spell.id,
      });
    }

    const setBtn = dialog.querySelector(".set");
    setBtn.onclick = () => {
      dialog.remove();
      const spell = picker.selected();
      console.log(spell);
      this.game.game.set_spellbook_spell(spell.id);
    };
    this.root.appendChild(dialog);
  }

  reportBug() {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Report Bug</div>
        <div slot="body">
          Click <a class="download">here</a> to download crash data. Send this to me along with a screenshot.
          <div class="right-button-container">
            <button class="ok">Ok</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const okBtn = dialog.querySelector(".ok");
    okBtn.onclick = () => dialog.remove();

    const downloadLink = dialog.querySelector(".download");
    const logs = this.game.getLogs();
    downloadLink.setAttribute(
      "href",
      "data:text/plain;charset=utf-8," + encodeURIComponent(logs),
    );
    downloadLink.setAttribute("download", `aldon_debug_logs_${Date.now()}`);

    this.root.appendChild(dialog);
  }

  preferences() {
    const scale = this.game.getScale();
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Preferences</div>
        <div slot="body">
          <div>
            <button class="scale-down">-</button>
            <button class="scale-up">+</button>
            Scale: <span class="scale-label">${scale}</span>
          </div>
          <div class="right-button-container">
            <button class="ok">Ok</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const scaleLabel = dialog.querySelector(".scale-label");

    const okBtn = dialog.querySelector(".ok");
    okBtn.onclick = () => dialog.remove();

    const scaleDown = dialog.querySelector(".scale-down");
    scaleDown.onclick = () => {
      let scale = this.game.getScale();
      scale = this.game.setScale(scale - 1);
      scaleLabel.innerText = `${scale}`;
    };
    const scaleUp = dialog.querySelector(".scale-up");
    scaleUp.onclick = () => {
      let scale = this.game.getScale();
      scale = this.game.setScale(scale + 1);
      scaleLabel.innerText = `${scale}`;
    };
    this.root.appendChild(dialog);
  }

  questLog() {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Log</div>
        <div slot="body">
          <aldon-picker> </aldon-picker>
          <div class="right-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
          </div>
        </div>
      </aldon-dialog>
    `);
    const picker = dialog.querySelector("aldon-picker");

    for (const quest of this.game.quests()) {
      picker.addItem(quest);
    }
    this.root.appendChild(dialog);
  }

  minimap() {
    const num_tiles = 24;
    const tile_size_px = 5;
    const scale = 5;
    const size = num_tiles * tile_size_px * scale;
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">MiniMap</div>
        <div slot="body">
          <canvas
            style="position: absolute; left: 50%; transform: translate(-50%, 0%)" width="${size}" height="${size}"
            class="mini-map">
          </canvas>
          <div class="right-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
          </div>
        </div>
      </aldon-dialog>
    `);
    const minimap = dialog.querySelector(".mini-map");
    this.game.game.draw_minimap(minimap, scale);
    this.root.appendChild(dialog);
  }

  downloadGame() {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Game Menu</div>
        <div slot="body">
          <aldon-picker></aldon-picker>
          <div class="right-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
            <button class="download">Download</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const picker = dialog.querySelector("aldon-picker");
    const store = this.game.getSaveStore();
    let saves = store.quicksave ? [store.quicksave] : [];
    saves = saves.concat(store.saves.filter((s) => s.data !== ""));

    for (const save of saves) {
      const name = saveName(save);
      picker.addItem({ name });
    }

    const downloadBtn = dialog.querySelector(".download");
    downloadBtn.onclick = () => {
      const slot = picker.selectedIndex();
      if (slot === -1) {
        return;
      }
      const save = saves[slot];
      const encoded = encodeURIComponent(save.data);
      const link = html(`
      <a
        style="display: none"
        download="${save.name}"
        href="data:text/plain;charset=utf-8,${encoded}"
      >
      </a>
    `);
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      dialog.remove();
    };
    this.root.appendChild(dialog);
  }

  saveGame() {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Game Menu</div>
        <div slot="body">
          <aldon-picker></aldon-picker>
          <div class="right-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
            <button class="save">Save</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const picker = dialog.querySelector("aldon-picker");
    const store = this.game.getSaveStore();
    for (const save of store.saves) {
      const name = saveName(save);
      picker.addItem({ name });
    }

    const saveBtn = dialog.querySelector(".save");
    saveBtn.onclick = () => {
      const slot = picker.selectedIndex();
      this.game.save(slot);
      dialog.remove();
    };
    this.root.appendChild(dialog);
  }

  loadGame() {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Game Menu</div>
        <div slot="body">
          <aldon-picker></aldon-picker>
          <div class="right-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
            <button class="from-file">From File</button>
            <button class="load">Load</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const picker = dialog.querySelector("aldon-picker");
    const store = this.game.getSaveStore();
    let saves = store.quicksave ? [store.quicksave] : [];
    saves = saves.concat(store.saves.filter((s) => s.data !== ""));

    for (const save of saves) {
      const name = saveName(save);
      picker.addItem({ name });
    }

    const loadBtn = dialog.querySelector(".load");
    loadBtn.onclick = () => {
      const slot = picker.selectedIndex();
      if (slot === -1) {
        return;
      }
      const save = saves[slot];
      this.game.loadSave(save);
      dialog.remove();
    };

    const fromFileBtn = dialog.querySelector(".from-file");
    fromFileBtn.onclick = () => {
      const fileInput = html(`<input type="file" style="display: none">`);
      fileInput.onchange = (event) => {
        const file = event.target.files[0];
        if (file) {
          const reader = new FileReader();
          reader.onload = (e) => {
            const data = e.target.result;
            console.log(data);
            this.game.loadSave({ data });
            dialog.remove();
          };
          reader.readAsText(file); // Read the file as a text string
          const gameContainer = document.getElementById("game-container");
          if (this.gameContainer.requestFullscreen) {
            this.gameContainer.requestFullscreen();
          }
        }
      };
      document.body.appendChild(fileInput);
      fileInput.click();
      document.body.removeChild(fileInput);
    };
    this.root.appendChild(dialog);
  }

  about() {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">About Aldon's Crossing</div>
        <div slot="body">
          <div>Aldon's Crossing</div>
          <div>A Constant Games Production.</div>
          <div>c 1999-2002</div>
          <div>Reimplemented by <a style="text-decoration: none" href="https://github.com/viruslobster/aldons-crossing-rust">@viruslobster</a></div>
          <div><img src="assets/qr.svg"></img></div>
          <div>Version: ${VERSION}</div>
          <div class="right-button-container">
            <button class="ok">OK</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const okBtn = dialog.querySelector(".ok");
    okBtn.onclick = () => dialog.remove();
    this.root.appendChild(dialog);
  }

  deleteGame() {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Game Menu</div>
        <div slot="body">
          <aldon-picker></aldon-picker>
          <div class="right-button-container">
            <aldon-dialog-done-button></aldon-dialog-done-button>
            <button class="delete">Delete</button>
          </div>
        </div>
      </aldon-dialog>
    `);

    const picker = dialog.querySelector("aldon-picker");
    const store = this.game.getSaveStore();

    let saves = store.quicksave ? [store.quicksave] : [];
    saves = saves.concat(store.saves.filter((s) => s.data !== ""));

    for (const save of saves) {
      const name = saveName(save);
      picker.addItem({ name });
    }

    const saveBtn = dialog.querySelector(".delete");
    saveBtn.onclick = () => {
      const i = picker.selectedIndex();
      if (i === 0 && store.quicksave) {
        this.game.deleteQuicksave();
        return;
      }
      const save = saves[i];
      this.game.deleteSave(save);
      dialog.remove();
    };
    this.root.appendChild(dialog);
  }

  notImplemented() {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Not Implemented</div>
        <div slot="body">
          This feature is not implemented yet.
          <div class="right-button-container">
            <button class="damn">Damn</button>
          </div>
        </div>
      </aldon-dialog>
    `);
    const damnBtn = dialog.querySelector(".damn");
    damnBtn.onclick = () => dialog.remove();
    this.root.appendChild(dialog);
  }
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

const dialogTemplate = document.createElement("template");
dialogTemplate.innerHTML = `
  <div class="window">
    <div class="title">
      <slot name="title"></slot>
    </div>
    <div class="body">
      <slot name="body"></slot>
    </div>
  </div>
`;

function saveName(save) {
  const time = save.time ? new Date(save.time).toLocaleString() : "";
  return `${save.name} ${time}`;
}

function withSign(num) {
  return num < 0 ? `${num}` : `+${num}`;
}

function statDetail(stat, value) {
  switch (stat) {
    case "str":
      const pctHit = Stats.strength_to_hit_bonus(value);
      const dmg = Stats.strength_to_damage(value);
      return `${withSign(pctHit)}%Hit,${withSign(dmg)} Dmg`;
    case "dex":
      const pctMsl = Stats.dexterity_to_hit_bonus(value);
      const ac = Stats.dexterity_to_armor_class(value);
      return `${withSign(pctMsl)}%Missile,${withSign(ac)} AC`;
    case "vit":
      const hp = Stats.vitality_to_hit_points(value);
      return `${withSign(hp)} HP per Lvl`;
    case "int":
      const cast = Stats.intelligence_to_chance_cast(value);
      return `Cast success:${cast}%`;
    case "wis":
      const mana = Stats.wisdom_to_mana(value);
      return `Base Mana:${mana}`;
    case "luck":
      const luck = Stats.luck_to_modifier(value);
      return `Modifier:${luck}%`;
    default:
      throw `No such stat ${stat}`;
  }
}

// Avoid aliasing problems with rust. These dialogs methods are called from
// rust, run as js, and make api calls back to rust. This recursive usage is
// not allowed by wasm-bindgen. So instead schedule the dialog method to run
// in a couple millis from js (not from rust).
const methods = Object.getOwnPropertyNames(Dialog.prototype);
const skipMethods = ["isOpen", "setGame", "resize", "constructor"];
for (const method of methods) {
  if (skipMethods.includes(method)) continue;

  const origional = Dialog.prototype[method];
  if (typeof origional !== "function") continue;

  Dialog.prototype[method] = function (...args) {
    setTimeout(() => origional.apply(this, args), 50);
  };
}

function capitalize(str) {
  return str[0].toUpperCase() + str.slice(1);
}

export { Dialog };
