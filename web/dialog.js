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

const userPortraits = [
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
    const left = width / 2 - (860 * scale) / 2 - border * scale;
    const style = document.createElement("style");

    style.textContent = `
      .window {
        width: ${860}px;
        height: ${860}px;
        transform: scale(${scale}, ${scale});
        top: ${top}px;
        left: ${left}px;

        transform-origin: top left;
        font-family: PalmOS;
        font-smooth: never;
        font-size: 80px;
        border-radius: 8px;
        position: absolute;
        border: ${border}px solid #2c008b;
        vertical-align: top;
        z-index: 2;
        background-color: white;
        overflow: hidden;
        touch-action: pan-y;
      }

      .window .title {
        font-family: PalmOSBold;
        font-size: 80px;
        background-color: #2c008b;
        text-align: center;
        color: white;
        border: 5px solid #2c008b;
        height: 10%;
      }

      .window .body {
        font-smooth: never;
        padding: 5px;
        height: 90%;
        touch-action: pan-y;
      }

      .button-container {
        position: absolute;
        bottom: 10px;
        font-size: 100px;
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

class Dialog {
  constructor(
    root,
    canvas,
    spritesheet,
    viewport_width,
    viewport_height,
    margin,
  ) {
    this.game = null;
    this.root = root;
    this.dialogOld = new DialogOld(
      root,
      spritesheet,
      viewport_width,
      viewport_height,
      margin,
    );
  }

  isOpen() {
    return this.dialogOld.isOpen();
  }

  setGame(game) {
    this.game = game;
    this.dialogOld.setGame(game.game);
  }

  resize(width, height) {
    this.dialogOld.resize(width, height);
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
    this.dialogOld.tellMessage(
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
    );
  }

  createCharacter() {
    this.dialogOld.createCharacter();
  }

  inventory(actorID, items) {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Transaction</div>
        <div slot="body">
          <aldon-picker previewPic="true" itemInfo="true">
          </aldon-picker>
          <div class="button-container">
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
          <div class="button-container">
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
          <div class="button-container">
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
          <div class="button-container">
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
    doneBtn.onclick = () => {
      this.game.untoggleMenuButton();
      dialog.remove();
      console.log("foo");
    };

    const setBtn = dialog.querySelector(".set");
    setBtn.onclick = () => {
      const idx = picker.selectedIndex();
      const button = buttons[idx];
      this.game.setButton(buttonIdx, button);
      dialog.remove();
      this.game.untoggleMenuButton();
    };
    this.root.appendChild(dialog);
  }

  stats(playerStats) {
    this.dialogOld.stats(playerStats);
  }

  spellbook(spells) {
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Transaction</div>
        <div slot="body">
          <aldon-picker previewPic="true" itemInfo="true">
          </aldon-picker>
          <div class="button-container">
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
          <div class="button-container">
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
    const scale = Number(localStorage.getItem("aldon-this.game-scale")) || 2;
    const dialog = html(`
      <aldon-dialog width="${this.game.width}" height="${this.game.height}">
        <div slot="title">Preferences</div>
        <div slot="body">
          <div>
            <button class="scale-down">-</button>
            <button class="scale-up">+</button>
            Scale: <span class="scale-label">${scale}</span>
          </div>
          <div class="button-container">
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
          <div class="button-container">
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
          <div class="button-container">
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
          <div class="button-container">
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
          <div class="button-container">
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
          <div class="button-container">
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
          <div>Reimplemented by <a href="https://github.com/viruslobster">@viruslobster</a></div>
          <div><img src="assets/qr.svg"></img></div>
          <div class="button-container">
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
          <div class="button-container">
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
          <div class="button-container">
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

// TODO: delete old style dialogs
function createWindow(
  type,
  titleStr,
  viewport_width,
  viewport_height,
  width,
  height,
  margin,
) {
  const body = document.createElement("div");
  body.setAttribute("class", "body");

  const title = document.createElement("div");
  title.setAttribute("class", "title");
  title.innerHTML = titleStr;

  const window = document.createElement("div");
  window.setAttribute("id", "window");
  window.setAttribute("class", `window ${type}`);
  window.appendChild(title);
  window.appendChild(body);
  window.style.margin = `${margin}px`;
  window.style.width = `${width - margin * 2}px`;
  window.style.height = `${height - margin * 2}px`;

  const adjusted_scale =
    (Math.min(viewport_width, viewport_height) - 3) / (430 * 2);
  window.style.transform = `scale(${adjusted_scale}, ${adjusted_scale})`;

  const scale = Math.min(viewport_width, viewport_height) / (430 * 2);
  const top = viewport_height / 2 - (height * scale) / 2;
  const left = viewport_width / 2 - (width * scale) / 2;
  window.style.top = `${top}px`;
  window.style.left = `${left}px`;
  return window;
}

// TODO: delete old style dialogs
function closeWindow() {
  const window = document.getElementById("window");
  if (window == null) throw "could not find window element";
  window.remove();
}

// TODO: delete old style dialogs
class DialogOld {
  constructor(root, spritesheet, viewport_width, viewport_height, margin) {
    this.game = null;
    this.root = root; // The node where windows will be attached
    this.spritesheet = spritesheet;
    this.dialogOpen = false;
    this.viewport_width = viewport_width;
    this.viewport_height = viewport_height;
    this.width = 430 * 2;
    this.height = 430 * 2;
    this.margin = margin;
    this.items = [];
    this.selected = 0;
    this.message = null;
    this.createCharacterDialog = new CreateCharacterDialog(
      root,
      spritesheet,
      viewport_width,
      viewport_height,
      this.width,
      this.height,
      this.scale,
      margin,
    );
    this.statsDialog = new StatsDialog(
      root,
      spritesheet,
      viewport_width,
      viewport_height,
      this.width,
      this.height,
      this.scale,
      margin,
    );
  }

  resize(width, height) {
    this.viewport_width = width;
    this.viewport_height = height;
    this.createCharacterDialog.resize(width, height);
    this.statsDialog.resize(width, height);
  }

  stats(stats) {
    this.statsDialog.open(stats);
  }

  setGame(game) {
    this.game = game;
    this.createCharacterDialog.game = game;
  }

  isOpen() {
    return (
      this.dialogOpen ||
      this.createCharacterDialog.isOpen() ||
      this.statsDialog.isOpen()
    );
  }

  selectedItem() {
    return this.items[this.selected];
  }

  popSelectedItem() {
    const item = this.selectedItem();
    this.items.splice(this.selected, 1);
    if (this.selected >= this.items.length) {
      this.selected = Math.max(this.items.length - 1, 0);
    }
    return item;
  }

  tellMessage(
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
    const choice = [choiceA, choiceB, choiceC];
    const pic = document.createElement("canvas");
    const scale = 8;
    pic.setAttribute("class", "picture");
    pic.setAttribute("width", 32 * scale);
    pic.setAttribute("height", 32 * scale);

    const ctx = pic.getContext("2d");
    if (ctx == null) throw "could not get 2d drawing context";
    ctx.imageSmoothingEnabled = false;
    ctx.scale(scale, scale);
    ctx.drawImage(
      this.spritesheet,
      portrait_x,
      portrait_y,
      portrait_w,
      portrait_h,
      0,
      0,
      portrait_w,
      portrait_h,
    );

    const responses = document.createElement("div");
    responses.setAttribute("class", "responses");
    const buttonNames = ["A", "B", "C"];
    for (let i = 0; i < 3; i++) {
      const container = document.createElement("div");
      const button = document.createElement("button");
      button.innerHTML = buttonNames[i];

      button.onclick = () => {
        closeWindow();
        setTimeout(() => {
          this.dialogOpen = false;
          this.game.send_response(fromActor, i);
        }, 400);
      };
      container.appendChild(button);
      const choiceMsg = choice[i];
      if (choiceMsg !== undefined) {
        container.appendChild(document.createTextNode(choiceMsg));
      } else {
        container.style.visibility = "hidden";
      }
      responses.appendChild(container);
    }

    const bodyStrDiv = document.createElement("div");
    bodyStrDiv.innerHTML = msg.replace(/&/gi, "<br />");

    const margin = 20;
    const window = createWindow(
      "actor",
      title,
      this.viewport_width,
      this.viewport_height,
      this.width,
      this.height,
      this.margin,
    );
    const body = window.querySelector(".body");
    body.appendChild(pic);
    body.appendChild(bodyStrDiv);
    body.appendChild(responses);
    this.root.appendChild(window);

    this.dialogOpen = true;
  }

  option(name, value) {
    const opt = document.createElement("option");
    opt.setAttribute("value", value.toString());
    opt.innerHTML = name;
    return opt;
  }

  createSellBtn(actorID, draw) {
    const btn = document.createElement("button");
    btn.innerHTML = "Sell";
    btn.onclick = () => {
      const ok = this.game.sell(actorID, this.selected);
      if (ok) {
        this.message = "Sold!";
        this.popSelectedItem();
        if (this.items.length === 0) {
          this.finishTransaction();
          return;
        }
      } else {
        this.message = "You can't sell this.";
      }
      draw();
    };
    return btn;
  }

  createBuyBtn(actorID, draw) {
    const btn = document.createElement("button");
    btn.innerHTML = "Buy";
    btn.onclick = () => {
      const ok = this.game.buy(actorID, this.selected);
      if (ok) {
        this.message = "Sold!";
        if (this.items.length === 0) {
          this.finishTransaction();
          return;
        }
      } else {
        this.message = "You can't pick this up.";
      }
      draw();
    };
    return btn;
  }

  createStatsBtn(actorID) {
    const btn = document.createElement("button");
    btn.innerHTML = "Stats";
    btn.onclick = () => {
      const stats = this.game.stats(actorID);
      console.log("show stats");
    };
    return btn;
  }

  createUnequipBtn(actorID, draw) {
    const item = this.selectedItem();
    const btn = document.createElement("button");
    if (true) {
      // TODO: if equiped
      btn.setAttribute("class", "hidden");
    }
    btn.innerHTML = "Unenquip";
    btn.onClick = () => {
      this.game.unequip(actorID, this.selected);
      draw();
    };
    return btn;
  }

  createEquipBtn(actorID, draw) {
    const btn = document.createElement("button");
    const item = this.selectedItem();
    const alreadyEquiped = this.game.is_equiped(this.selected);
    console.log(`equiped = ${alreadyEquiped}`);

    btn.innerHTML = alreadyEquiped ? "Unequip" : "Equip";
    if (alreadyEquiped) {
      btn.onclick = () => {
        const ok = this.game.unequip(actorID, this.selected);
        if (!ok) {
          this.message = "Can't unequip";
        }
        draw();
      };
    } else {
      btn.onclick = () => {
        const ok = this.game.equip(actorID, this.selected);
        if (!ok) {
          this.message = "Can't equip";
        }
        draw();
      };
    }
    return btn;
  }

  createDropBtn(actorID, draw) {
    const btn = document.createElement("button");
    btn.innerHTML = "Drop";
    btn.onclick = () => {
      console.log(`Attempting to drop ${this.selectedItem()}`);
      console.log(this.selectedItem());
      const ok = this.game.drop(actorID, this.selected);
      if (ok) {
        this.popSelectedItem();
        if (this.items.length === 0) {
          this.finishTransaction();
          return;
        }
      } else {
        this.message = "Can't drop";
      }
      draw();
    };
    return btn;
  }

  createPickUpBtn(actorID, draw) {
    const btn = document.createElement("button");
    btn.innerHTML = "Pick Up";
    btn.onclick = () => {
      const ok = this.game.pickup(actorID, this.selected);
      if (ok) {
        this.popSelectedItem();
        if (this.items.length === 0) {
          this.finishTransaction();
          return;
        }
      } else {
        this.message = "You can't pick this up";
      }
      console.log(`selected: ${this.selected}`);
      draw();
    };
    return btn;
  }

  createDoneBtn() {
    const btn = document.createElement("button");
    btn.innerHTML = "Done";
    btn.onclick = () => {
      this.finishTransaction();
    };
    return btn;
  }

  finishTransaction() {
    closeWindow();
    setTimeout(() => {
      this.dialogOpen = false;
    }, 100);
  }

  createCharacter() {
    this.createCharacterDialog.open();
  }

  showStats() {
    this.statsDialog.open();
  }
}

// TODO: delete old style dialogs
class CreateCharacterDialog {
  constructor(
    root,
    spritesheet,
    viewport_width,
    viewport_height,
    width,
    height,
    scale,
    margin,
  ) {
    this.game = null;
    this.root = root;
    this.spritesheet = spritesheet;
    this.width = width;
    this.height = height;
    this.scale = scale;
    this.margin = margin;
    this.dialogOpen = false;
    this.viewport_width = viewport_width;
    this.viewport_height = viewport_height;

    this.portrait_idx = 0;
    this.points = 12;
    this.boundsByRace = {
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
    this.defaultByRace = {
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

    this.stats = { ...this.defaultByRace.human };
    this.race = "human";
  }

  resize(width, height) {
    this.viewport_width = width;
    this.viewport_height = height;
  }

  open() {
    const window = createWindow(
      "character-creation",
      "Character Creation",
      this.viewport_width,
      this.viewport_height,
      this.width,
      this.height,
      this.margin,
    );
    const body = window.querySelector(".body");
    this.root.appendChild(window);

    const nameInput = document.createElement("input");
    nameInput.setAttribute("maxlength", "14");
    nameInput.setAttribute("type", "text");
    nameInput.setAttribute("placeholder", "Enter Name");
    body.appendChild(nameInput);
    const pic = document.createElement("canvas");
    body.appendChild(pic);
    const scale = 6;
    pic.setAttribute("class", "picture");
    pic.setAttribute("width", 32 * scale);
    pic.setAttribute("height", 32 * scale);
    const ctx = pic.getContext("2d");
    ctx.imageSmoothingEnabled = false;
    ctx.scale(scale, scale);

    const drawPic = () => {
      const x = userPortraits[this.portrait_idx].x;
      const y = userPortraits[this.portrait_idx].y;
      ctx.drawImage(this.spritesheet, x, y, 32, 32, 0, 0, 32, 32);
    };

    const prevPicBtn = document.createElement("button");
    prevPicBtn.innerHTML = "<";
    body.appendChild(prevPicBtn);
    prevPicBtn.onclick = () => {
      this.portrait_idx--;
      if (this.portrait_idx < 0) {
        this.portrait_idx = userPortraits.length - 1;
      }
      draw();
    };

    const nextPicBtn = document.createElement("button");
    nextPicBtn.innerHTML = ">";
    body.appendChild(nextPicBtn);
    nextPicBtn.onclick = () => {
      this.portrait_idx++;
      if (this.portrait_idx >= userPortraits.length) {
        this.portrait_idx = 0;
      }
      draw();
    };

    const raceDiv = document.createElement("div");
    raceDiv.setAttribute("class", "race-container");
    body.appendChild(raceDiv);

    const nextRaceBtn = document.createElement("button");
    nextRaceBtn.innerHTML = ">";
    raceDiv.appendChild(nextRaceBtn);

    const raceTxt = document.createElement("div");
    raceDiv.appendChild(raceTxt);

    const drawRace = () => {
      const raceName = this.race.charAt(0).toUpperCase() + this.race.slice(1);
      raceTxt.innerHTML = `Race:${raceName}`;
    };

    const pointsDiv = document.createElement("div");
    const drawPoints = () => {
      pointsDiv.innerHTML = `Points:${this.points}`;
    };

    const statsDiv = document.createElement("div");
    body.appendChild(statsDiv);

    const drawStats = () => {
      statsDiv.innerHTML = "";

      for (const [stat, value] of Object.entries(this.stats)) {
        const row = this.createStatRow(stat, draw);
        statsDiv.appendChild(row);
      }
    };

    const draw = () => {
      drawPic();
      drawPoints();
      drawRace();
      drawStats();
    };

    body.appendChild(pointsDiv);

    const doneBtn = document.createElement("button");
    doneBtn.setAttribute("class", "done-button");
    body.appendChild(doneBtn);
    doneBtn.innerHTML = "Done";
    doneBtn.onclick = () => {
      closeWindow();
      const portrait = userPortraits[this.portrait_idx].id;
      this.game.new_game(
        nameInput.value,
        this.race,
        portrait,
        this.stats.str,
        this.stats.dex,
        this.stats.vit,
        this.stats.int,
        this.stats.wis,
        this.stats.luck,
      );

      setTimeout(() => {
        this.dialogOpen = false;
      }, 400);
    };

    nextRaceBtn.onclick = () => {
      switch (this.race) {
        case "human":
          this.race = "dwarf";
          break;
        case "dwarf":
          this.race = "elf";
          break;
        case "elf":
          this.race = "human";
          break;
        default:
          throw `Unknown race ${this.race}`;
      }
      this.stats = { ...this.defaultByRace[this.race] };
      this.points = 12;
      draw();
    };

    draw();
    this.dialogOpen = true;
  }

  incrementStat(stat) {
    if (this.points <= 0) {
      return;
    }
    const [_, max] = this.boundsByRace[this.race][stat];
    if (this.stats[stat] >= max) {
      return;
    }
    this.points--;
    this.stats[stat]++;
  }

  decrementStat(stat) {
    const [min, _] = this.boundsByRace[this.race][stat];
    if (this.stats[stat] <= min) {
      return;
    }
    this.points++;
    this.stats[stat]--;
  }

  createStatRow(stat, draw) {
    const containerDiv = document.createElement("div");
    containerDiv.setAttribute("class", "stat-row");

    const minusBtn = document.createElement("button");
    minusBtn.innerHTML = "-";
    containerDiv.appendChild(minusBtn);

    const plusBtn = document.createElement("button");
    plusBtn.innerHTML = "+";
    containerDiv.appendChild(plusBtn);

    const textDiv = document.createElement("div");
    textDiv.setAttribute("class", "stat-row-label");
    containerDiv.appendChild(textDiv);

    const statName = stat.charAt(0).toUpperCase() + stat.slice(1);
    const detail = statDetail(stat, this.stats[stat]);
    textDiv.innerHTML = `${statName}:${this.stats[stat]} (${detail})`;

    minusBtn.onclick = () => {
      this.decrementStat(stat);
      draw();
    };

    plusBtn.onclick = () => {
      this.incrementStat(stat);
      draw();
    };
    return containerDiv;
  }

  isOpen() {
    return this.dialogOpen;
  }
}

// TODO: delete old style dialogs
class BaseDialog {
  constructor(
    root,
    spritesheet,
    viewport_width,
    viewport_height,
    width,
    height,
    scale,
    margin,
  ) {
    this.game = null;
    this.root = root;
    this.spritesheet = spritesheet;
    this.width = width;
    this.height = height;
    this.scale = scale;
    this.margin = margin;
    this.dialogOpen = false;
    this.viewport_width = viewport_width;
    this.viewport_height = viewport_height;
    this.window_open = false;
  }

  resize(width, height) {
    this.viewport_width = width;
    this.viewport_height = height;
  }

  isOpen() {
    return this.window_open;
  }

  open() {
    console.log(`Size: ${this.viewport_width} x ${this.viewport_height}`);
    const window = createWindow(
      "character-stats",
      "Stats",
      this.viewport_width,
      this.viewport_height,
      this.width,
      this.height,
      this.margin,
    );
    this.root.appendChild(window);
    this.window_open = true;
    this.render();
  }

  close() {
    const window = document.getElementById("window");
    if (window == null) throw "could not find window element";
    window.remove();
    this.window_open = false;
  }

  render() {
    if (!this.window_open) {
      return;
    }
    const window = document.getElementById("window");
    const body = window.querySelector(".body");
    body.innerHTML = `<h1>Hi</h1>`;
  }
}

// TODO: delete old style dialogs
class StatsDialog extends BaseDialog {
  constructor(
    root,
    spritesheet,
    viewport_width,
    viewport_height,
    width,
    height,
    scale,
    margin,
  ) {
    super(
      root,
      spritesheet,
      viewport_width,
      viewport_height,
      width,
      height,
      scale,
      margin,
    );
  }

  open(stats) {
    this.stats = stats;
    super.open();
  }

  render() {
    if (!this.window_open) {
      return;
    }
    const window = document.getElementById("window");
    const body = window.querySelector(".body");
    const scale = 7;
    const gp_scale = 4;

    const style_row_item = "width: 30%; display: inline-block";
    const style_done_button = `
        position: fixed;
        bottom: 10px;
        right: 10px;
    `;
    const style_gp = `
      display: inline;
    `;
    const can_level_up = Stats.max_level(this.stats.exp) > this.stats.level;

    body.innerHTML = `
      <div style="overflow: auto">
        <canvas style="float: right" class="picture" width="${32 * scale}" height="${32 * scale}"></canvas>
        <div>${this.stats.name}</div>
        <div>Class: ${this.stats.klass}</div>
        <div>Race: ${this.stats.race}</div>
      </div>
      <div>
        <span style="${style_row_item}">Lvl: ${this.stats.level}${
          can_level_up ? "+" : ""
        }</span>
        <span style="${style_row_item}">HP: ${this.stats.hp}/${
          this.stats.hp_max
        }</span>
        <span style="${style_row_item}">AC: ${this.stats.ac}</span>
      </div>
      <div style="display: flex">
        <span style="${style_row_item}">EXP: ${this.stats.exp}</span>
        <span style="${style_row_item}">MP: ${this.stats.mp}/${
          this.stats.mp_max
        }</span>
        <span style="width: 30%; display: inline-flex">
          <canvas id="gp-pic" style="${style_gp}" width="${16 * gp_scale}" height="${16 * gp_scale}"></canvas>
          <span style="">: ${this.stats.gp}</span>
        <span>
      </div>
      <div>Str:${this.stats.str} (${statDetail("str", this.stats.str)})</div>
      <div>Dex:${this.stats.dex} (${statDetail("dex", this.stats.dex)})</div>
      <div>Vit:${this.stats.vit} (${statDetail("vit", this.stats.vit)})</div>
      <div>Int:${this.stats.int} (${statDetail("int", this.stats.int)})</div>
      <div>Wis:${this.stats.wis} (${statDetail("wis", this.stats.wis)})</div>
      <div>Luck:${this.stats.luck} (${statDetail(
        "luck",
        this.stats.luck,
      )})</div>
      
      <button style="${style_done_button}" class="closeButton">Done</button>
    `;
    body.querySelector(".closeButton").onclick = () => this.close();

    const ctx = body.querySelector(".picture").getContext("2d");
    ctx.imageSmoothingEnabled = false;
    ctx.scale(scale, scale);
    const portrait_idx =
      portraits.findIndex((p) => p.id == this.stats.portrait) || 0;
    const x = portraits[portrait_idx].x;
    const y = portraits[portrait_idx].y;
    ctx.drawImage(this.spritesheet, x, y, 32, 32, 0, 0, 32, 32);

    const gp_ctx = body.querySelector("#gp-pic").getContext("2d");
    gp_ctx.imageSmoothingEnabled = false;
    gp_ctx.scale(gp_scale, gp_scale);
    gp_ctx.drawImage(this.spritesheet, 495, 179, 16, 16, 0, 0, 16, 16);
  }
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
  console.log(method);

  Dialog.prototype[method] = function (...args) {
    console.log("here");
    console.log(this);
    setTimeout(() => origional.apply(this, args), 50);
  };
}

export { Dialog };
