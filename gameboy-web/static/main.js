// ─────────────────────────────────────────────────────────────
//  Game Boy Web Emulator — main entry-point
//  Loads WASM module, handles rendering, input, and audio.
// ─────────────────────────────────────────────────────────────

import init, { GameBoyWeb } from "./pkg/gameboy_web.js";

// ── Constants ───────────────────────────────────────────────

/** Keyboard key → Game Boy button ID (matches map_button() in Rust). */
const KEY_MAP = {
    ArrowRight: 0,
    ArrowLeft:  1,
    ArrowUp:    2,
    ArrowDown:  3,
    KeyZ:       4,   // B
    KeyX:       5,   // A
    Enter:      6,   // Start
    Backspace:  7,   // Select
};

// ── State ───────────────────────────────────────────────────

/** @type {GameBoyWeb | null} */
let emulator = null;

/** @type {CanvasRenderingContext2D | null} */
let ctx = null;

/** @type {ImageData | null} */
let imageData = null;

/** WASM linear memory reference. */
let wasmMemory = null;

/** Audio context (created on first user gesture). */
let audioCtx = null;

/** Audio worklet node. */
let audioWorkletNode = null;

/** Whether audio has been unlocked by a user gesture. */
let audioUnlocked = false;

/** Frame-counter for FPS display. */
let frameCount = 0;
let lastFpsTime = performance.now();

// ── DOM references ──────────────────────────────────────────

const canvas        = document.getElementById("screen");
const screenOverlay = document.getElementById("screen-overlay");
const romInput      = document.getElementById("rom-input");
const romLabel      = document.getElementById("rom-label");
const romInfo       = document.getElementById("rom-info");
const fpsDisplay    = document.getElementById("fps-display");
const audioStatus   = document.getElementById("audio-status");
const audioNotice   = document.getElementById("audio-notice");
const powerLed      = document.getElementById("power-led");
const actionBtns    = document.querySelectorAll(".action-btn");
const startBtn      = document.querySelector(".start-btn");
const selectBtn     = document.querySelector(".select-btn");

// ── Bootstrap ───────────────────────────────────────────────

async function main() {
    // 1. Load & instantiate the WASM module
    const wasm = await init();
    wasmMemory = wasm.memory;

    // 2. Create emulator
    emulator = new GameBoyWeb();

    // 3. Set-up canvas
    ctx = canvas.getContext("2d", { willReadFrequently: true });

    // 4. Wire-up file input
    romInput.addEventListener("change", onRomSelected);

    // 5. Wire-up keyboard
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup",   onKeyUp);

    // 5b. Wire-up on-screen buttons
    actionBtns.forEach(btn => {
        btn.addEventListener("pointerdown", (e) => {
            e.preventDefault();
            const btnId = parseInt(btn.dataset.btn);
            emulator?.key_down(btnId);
            btn.classList.add("active");
        });
        btn.addEventListener("pointerup", (e) => {
            e.preventDefault();
            const btnId = parseInt(btn.dataset.btn);
            emulator?.key_up(btnId);
            btn.classList.remove("active");
        });
        btn.addEventListener("pointerleave", (e) => {
            const btnId = parseInt(btn.dataset.btn);
            emulator?.key_up(btnId);
            btn.classList.remove("active");
        });
    });

    if (startBtn) {
        startBtn.addEventListener("pointerdown", (e) => {
            e.preventDefault();
            emulator?.key_down(6);
            startBtn.classList.add("active");
        });
        startBtn.addEventListener("pointerup", (e) => {
            e.preventDefault();
            emulator?.key_up(6);
            startBtn.classList.remove("active");
        });
        startBtn.addEventListener("pointerleave", () => {
            emulator?.key_up(6);
            startBtn.classList.remove("active");
        });
    }

    if (selectBtn) {
        selectBtn.addEventListener("pointerdown", (e) => {
            e.preventDefault();
            emulator?.key_down(7);
            selectBtn.classList.add("active");
        });
        selectBtn.addEventListener("pointerup", (e) => {
            e.preventDefault();
            emulator?.key_up(7);
            selectBtn.classList.remove("active");
        });
        selectBtn.addEventListener("pointerleave", () => {
            emulator?.key_up(7);
            selectBtn.classList.remove("active");
        });
    }

    // 6. Unlock audio on first user gesture
    const unlockAudio = async () => {
        if (!audioUnlocked) {
            await initAudio();
            audioUnlocked = true;
            audioNotice.classList.add("hidden");
        }
    };
    window.addEventListener("click",    unlockAudio, { once: false });
    window.addEventListener("keydown",  unlockAudio, { once: false });

    console.log("[GB-Web] Ready.  Load a .gb ROM to start.");
}

main().catch(console.error);

// ── ROM loading ─────────────────────────────────────────────

/** @param {Event} e */
function onRomSelected(e) {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = () => {
        const bytes = new Uint8Array(reader.result);
        try {
            const title = emulator.load_rom(bytes);
            console.log(`[GB-Web] ROM loaded: ${title}`);

            // Update UI
            romInfo.innerHTML = `<span class="badge loaded">${title || file.name}</span>`;
            romLabel.querySelector("span").textContent = file.name;
            screenOverlay.classList.add("hidden");
            powerLed.classList.add("on");

            // Start the game loop (idempotent)
            requestAnimationFrame(gameLoop);
        } catch (err) {
            alert("Failed to load ROM: " + err);
            console.error(err);
        }
    };
    reader.readAsArrayBuffer(file);
}

// ── Game loop ───────────────────────────────────────────────

function gameLoop(_timestamp) {
    if (!emulator || !emulator.is_rom_loaded()) {
        requestAnimationFrame(gameLoop);
        return;
    }

    // 1. Run one frame of emulation
    emulator.run_frame();

    // 2. Render frame buffer to canvas
    renderFrame();

    // 3. Push audio samples to worklet
    pushAudio();

    // 4. FPS counter
    frameCount++;
    const now = performance.now();
    if (now - lastFpsTime >= 1000) {
        fpsDisplay.textContent = frameCount.toString();
        frameCount = 0;
        lastFpsTime = now;
    }

    // 5. Schedule next frame
    requestAnimationFrame(gameLoop);
}

// ── Rendering ───────────────────────────────────────────────

function renderFrame() {
    const ptr = emulator.frame_buffer_ptr();
    const len = emulator.frame_buffer_len();

    // Create a clamped view over WASM memory — zero copy
    const pixels = new Uint8ClampedArray(wasmMemory.buffer, ptr, len);

    const w = emulator.screen_width();
    const h = emulator.screen_height();

    // Re-create ImageData only if dimensions changed (shouldn't in practice)
    if (!imageData || imageData.width !== w || imageData.height !== h) {
        imageData = new ImageData(w, h);
    }

    // Copy pixel data (ImageData owns its buffer, so we must copy)
    imageData.data.set(pixels);

    ctx.putImageData(imageData, 0, 0);
}

// ── Audio ───────────────────────────────────────────────────

async function initAudio() {
    try {
        if (!audioCtx) {
            audioCtx = new AudioContext({ sampleRate: 44100 });
            await audioCtx.audioWorklet.addModule("audio-worklet.js");
        }

        if (audioCtx.state === "suspended") {
            await audioCtx.resume();
        }

        if (!audioWorkletNode) {
            audioWorkletNode = new AudioWorkletNode(audioCtx, "gb-audio-processor", {
                outputChannelCount: [2],
            });
            audioWorkletNode.connect(audioCtx.destination);
        }

        audioStatus.textContent = "On";
        audioStatus.style.color = "var(--accent)";
        console.log("[GB-Web] Audio initialised (AudioWorklet, 44100 Hz).");
    } catch (err) {
        console.warn("[GB-Web] AudioWorklet failed, audio disabled:", err);
        audioStatus.textContent = "Error";
        audioStatus.style.color = "var(--danger)";
    }
}

function pushAudio() {
    if (!audioWorkletNode || !audioCtx || audioCtx.state !== "running") return;

    const floatCount = emulator.drain_audio_samples();
    if (floatCount === 0) return;

    const ptr = emulator.audio_staging_ptr();
    // Copy from WASM memory into a fresh Float32Array (must copy for postMessage)
    const view = new Float32Array(wasmMemory.buffer, ptr, floatCount);
    const copy = new Float32Array(view);

    audioWorkletNode.port.postMessage({ type: "samples", samples: copy });
}

// ── Input ───────────────────────────────────────────────────

/** @param {KeyboardEvent} e */
function onKeyDown(e) {
    const btn = KEY_MAP[e.code];
    if (btn !== undefined) {
        e.preventDefault();
        emulator?.key_down(btn);
    }
}

/** @param {KeyboardEvent} e */
function onKeyUp(e) {
    const btn = KEY_MAP[e.code];
    if (btn !== undefined) {
        e.preventDefault();
        emulator?.key_up(btn);
    }
}
