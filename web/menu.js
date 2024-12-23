// The drop down menu at the top of the game. Made to resemble system menu on old Palm devices.
import {
  aldonSaveGameDialog,
  aldonLoadGameDialog,
  aldonDeleteGameDialog,
  aldonAboutDialog,
  aldonMiniMapDialog,
  aldonNotImplementedDialog,
  aldonReportBugDialog,
  aldonPreferencesDialog,
  aldonDownloadGameDialog,
  aldonQuestLogDialog,
} from "./dialog.js";

/**
 * You might think we can use a <template> instead of a <div> here
 * but that doesn't work if you're trying to use custom elements.
 */
function html(content) {
  const el = document.createElement("div");
  el.innerHTML = content.trim();
  return el.firstChild;
}

class AldonMenu extends HTMLElement {
  constructor() {
    super();
    this.game = null;
    const style = document.createElement("style");

    style.textContent = `
      .menu-container {
        background-color: white;
        z-index: 10;
        font-family: PalmOSBold;
        font-size: 5vh; //2rem; // 5rem
        border: 2px;
        border-style: solid;
        border-radius: 5px;
        padding-left: 10px;
      }

      @media only screen and (orientation: landscape) {
        .menu-container {
          font-size: 10vh;
        }
      }

      .menu {
        margin-right: 10px;
        position: relative;
        display: inline-block;
      }

      .menu-content {
        color: black;
        display: none;
        position: absolute;
        background-color: white;
        border: 2px;
        border-radius: 5px;
        border-style: solid;
        box-shadow: 4px 4px black;
        white-space: nowrap;
        z-index: 3;
      }

      .menu-content div {
        padding-left: 10px;
        padding-right: 10px;
        margin-top: 1vh;
        margin-bottom: 1vh;
      }

      .menu-content div:active {
        background-color: #2c008b;
        color: white;
      }

      .menu-content div:hover {
        background-color: #2c008b;
        color: white;
      }

      .menu.open .menu-content {
        display: block;
      }

      .menu.open {
        background-color: #2c008b;
        color: white;
      }
    `;
    const root = html(`
      <div class="menu-container">
        <div class="menu">
          <span>Menu</span>
          <div class="menu-content">
            <div id="quick-save" class="show-when-playing">QuickSave</div>
            <div id="load">Load</div>
            <div id="save" class="show-when-playing">Save</div>
            <div id="delete">Delete</div>
            <div id="download">Download</div>
            <div id="new">New</div>
          </div>
        </div>
        <div class="menu">
          <span>Options</span>
          <div class="menu-content">
            <div id="quest-log" class="show-when-playing">
              Quest Log
            </div>
            <div id="mini-map" class="show-when-playing">MiniMap</div>
            <div id="message-log" class="show-when-playing">
              MessageLog
            </div>
            <div id="preferences">
              Preferences
            </div>
            <div id="report-bug">Report Bug</div>
            <div id="about">
              About Aldon's Crossing
            </div>
          </div>
      </div>
    `);
    root.querySelectorAll("div.menu").forEach((div) => {
      div.addEventListener("touchstart", () => {
        this.update();
        div.classList.add("open");
      });
      div.onmouseover = () => {
        this.update();
        div.classList.add("open");
      };
      div.onmouseout = () => div.classList.remove("open");
    });
    root
      .querySelectorAll(".menu-content > div")
      .forEach((div) => div.addEventListener("click", () => this.close()));
    root
      .querySelector("#quick-save")
      .addEventListener("click", () => this.game.quicksave());
    root.querySelector("#load").onclick = () => {
      let dialog = aldonLoadGameDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#save").onclick = () => {
      let dialog = aldonSaveGameDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#delete").onclick = () => {
      let dialog = aldonDeleteGameDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#download").onclick = () => {
      let dialog = aldonDownloadGameDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#new").onclick = () => {
      this.game.dialog.createCharacter();
    };
    root.querySelector("#quest-log").onclick = () => {
      let dialog = aldonQuestLogDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#mini-map").onclick = () => {
      let dialog = aldonMiniMapDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#message-log").onclick = () => {
      let dialog = aldonNotImplementedDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#preferences").onclick = () => {
      let dialog = aldonPreferencesDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#report-bug").onclick = () => {
      let dialog = aldonReportBugDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    root.querySelector("#about").onclick = () => {
      let dialog = aldonAboutDialog(this.game);
      document.getElementById("game").appendChild(dialog);
    };
    const shadowRoot = this.attachShadow({ mode: "open" });
    shadowRoot.appendChild(style);
    shadowRoot.appendChild(root);
  }

  close() {
    this.shadowRoot
      .querySelectorAll("div.menu")
      .forEach((div) => div.classList.remove("open"));
  }

  update() {
    if (this.game === null) {
      this.shadowRoot.querySelectorAll(".menu-content > div").forEach((div) => {
        div.style.display = "none";
      });
      return;
    }
    this.shadowRoot.querySelectorAll(".menu-content > div").forEach((div) => {
      div.style.display = "block";
    });

    if (!this.game.playing()) {
      this.shadowRoot.querySelectorAll(".show-when-playing").forEach((div) => {
        div.style.display = "none";
      });
    }
  }
}
customElements.define("aldon-menu", AldonMenu);
